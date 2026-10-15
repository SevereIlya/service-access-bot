use crate::adapters::{
    StubText,
    web::{WebError, WebResult, WebState},
};
use crate::application::DeviceAccess;
use crate::domain::device::DeviceSignal;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use tracing::instrument;

const MAX_HWID_CHARS: usize = 128;
const MAX_OS_NAME_CHARS: usize = 64;
const MAX_OS_VERSION_CHARS: usize = 64;
const MAX_MODEL_CHARS: usize = 128;
const MAX_USER_AGENT_CHARS: usize = 256;
const PROFILE_UPDATE_HOURS: &str = "6";

#[derive(Debug, PartialEq, Eq)]
enum ProfileReply {
    Config,
    Stub(String),
    Reject,
}

/// Обрабатывает запрос подписки и возвращает конфиг либо заглушку.
///
/// Ищет пользователя по токену подписки из ссылки, записывает сигнал устройства
/// и по результату доступа отдаёт либо рабочий конфиг, либо заглушку.
///
/// # Возвращаемое значение
///
/// Ответ с конфигом (YAML) или заглушкой.
///
/// # Ошибки
///
/// [`WebError::NotFound`], если пользователь по токену не найден.
#[instrument(skip(state, headers, token))]
pub async fn handle_subscription_profile(
    State(state): State<WebState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> WebResult<Response> {
    let user = state
        .usecases
        .get_user_by_subscription_token
        .execute(&token)
        .await?
        .ok_or(WebError::NotFound)?;

    let access = state.usecases.register_device.execute(&user, device_signal(&headers)).await?;

    let yaml = match reply_for(&access, &state.stub_text) {
        ProfileReply::Config => state.usecases.issue_vpn_config.execute(&user).await?,
        ProfileReply::Stub(message) => state.usecases.get_stub_profile.execute(&[message])?,
        ProfileReply::Reject => return Err(WebError::Forbidden),
    };

    let filename = format!("ParalinkVPN-{}.yaml", user.telegram_id().inner());

    #[rustfmt::skip]
    let response_headers = [
        (header::CONTENT_TYPE, "text/yaml; charset=utf-8".to_string()),
        (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate".to_string()),
        (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        (header::HeaderName::from_static("profile-update-interval"), PROFILE_UPDATE_HOURS.to_string()),
    ];

    Ok((response_headers, yaml).into_response())
}

/// Определяет ответ на запрос по результату проверки доступа.
///
/// Возвращает рабочий конфиг (`ProfileReply::Config`) для [`DeviceAccess::Granted`),
/// отказ (`ProfileReply::Reject`) для [`DeviceAccess::RejectedClient`]
/// или заглушку (`ProfileReply::Stub`) для остальных исходов.
///
/// Текст заглушки берётся из [`StubText`].
///
/// # Замечания
///
/// `match` без ветки `_ =>`. При добавлении нового варианта [`DeviceAccess`] компилятор потребует описать.
fn reply_for(access: &DeviceAccess, texts: &StubText) -> ProfileReply {
    let message = match access {
        DeviceAccess::Granted => return ProfileReply::Config,
        DeviceAccess::RejectedClient => return ProfileReply::Reject,
        DeviceAccess::NoActiveSubscription => texts.stub_no_subscription.clone(),
        DeviceAccess::NeedsHwid => texts.stub_needs_hwid.clone(),
        DeviceAccess::NotRegistered => texts.stub_not_registered.clone(),
        DeviceAccess::Revoked => texts.stub_revoked.clone(),
        DeviceAccess::UnsupportedClient => texts.stub_unsupported_client.clone(),
        DeviceAccess::AwaitingSetup { code } => {
            texts.stub_awaiting_setup.replace("{code}", code.inner())
        }
        DeviceAccess::LimitExceeded { limit, current } => texts
            .stub_limit_exceeded
            .replace("{current}", &current.to_string())
            .replace("{limit}", &limit.to_string()),
    };
    ProfileReply::Stub(message)
}

/// Собирает сигнал устройства из заголовков запроса.
///
/// Каждое значение извлекается через [`header_text`] с ограничением длины.
/// Сомнительные заголовки обрезаются ещё до попадания в домен и БД.
fn device_signal(headers: &HeaderMap) -> DeviceSignal {
    DeviceSignal::new(
        header_text(headers, "x-hwid", MAX_HWID_CHARS),
        header_text(headers, "x-device-os", MAX_OS_NAME_CHARS),
        header_text(headers, "x-ver-os", MAX_OS_VERSION_CHARS),
        header_text(headers, "x-device-model", MAX_MODEL_CHARS),
        header_text(headers, header::USER_AGENT.as_str(), MAX_USER_AGENT_CHARS),
    )
}

/// Значение заголовка как текст: без пробелов по краям, не длиннее `max_chars`, пустое значение считается отсутствующим.
///
/// # Возвращаемое значение
///
/// `Some(text)` - непустое обрезанное значение;
/// `None` - заголовок отсутствует или пуст.
fn header_text(headers: &HeaderMap, key: &str, max_chars: usize) -> Option<String> {
    let value = headers.get(key)?;
    let text: String =
        String::from_utf8_lossy(value.as_bytes()).trim().chars().take(max_chars).collect();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::device::DeviceSetupCode;
    use axum::http::HeaderValue;

    const CODE: &str = "0123456789abcdef0123456789abcdef";

    fn texts() -> StubText {
        StubText {
            stub_no_subscription: "no-sub".to_owned(),
            stub_needs_hwid: "needs-hwid".to_owned(),
            stub_awaiting_setup: "setup {code}".to_owned(),
            stub_not_registered: "not-registered".to_owned(),
            stub_revoked: "revoked".to_owned(),
            stub_unsupported_client: "unsupported".to_owned(),
            stub_limit_exceeded: "limit {current}/{limit}".to_owned(),
        }
    }

    // ==========================================
    // reply_for
    // ==========================================

    #[test]
    fn granted_serves_the_working_config() {
        assert_eq!(reply_for(&DeviceAccess::Granted, &texts()), ProfileReply::Config);
    }

    #[test]
    fn rejected_client_gets_no_stub_at_all() {
        assert_eq!(reply_for(&DeviceAccess::RejectedClient, &texts()), ProfileReply::Reject);
    }

    #[test]
    fn every_other_outcome_gets_its_own_stub_message() {
        let code = DeviceSetupCode::parse(CODE).unwrap();
        let cases = [
            (DeviceAccess::NoActiveSubscription, "no-sub".to_owned()),
            (DeviceAccess::NeedsHwid, "needs-hwid".to_owned()),
            (DeviceAccess::NotRegistered, "not-registered".to_owned()),
            (DeviceAccess::Revoked, "revoked".to_owned()),
            (DeviceAccess::UnsupportedClient, "unsupported".to_owned()),
            (DeviceAccess::AwaitingSetup { code }, format!("setup {CODE}")),
            (DeviceAccess::LimitExceeded { limit: 3, current: 3 }, "limit 3/3".to_owned()),
        ];

        for (access, expected) in cases {
            assert_eq!(reply_for(&access, &texts()), ProfileReply::Stub(expected), "{access:?}");
        }
    }

    // ==========================================
    // header_text и device_signal
    // ==========================================

    #[test]
    fn header_text_caps_by_chars_not_bytes() {
        let mut headers = HeaderMap::new();
        let long = "я".repeat(200);
        headers.insert("x-hwid", HeaderValue::from_bytes(long.as_bytes()).unwrap());

        let capped = header_text(&headers, "x-hwid", 128).unwrap();

        assert_eq!(capped.chars().count(), 128);
    }

    #[test]
    fn header_text_trims_and_drops_blank_values() {
        let mut headers = HeaderMap::new();
        headers.insert("x-hwid", HeaderValue::from_static("  hw-1  "));
        headers.insert("x-ver-os", HeaderValue::from_static("   "));

        assert_eq!(header_text(&headers, "x-hwid", 64).as_deref(), Some("hw-1"));
        assert_eq!(header_text(&headers, "x-ver-os", 64), None);
        assert_eq!(header_text(&headers, "x-missing", 64), None);
    }

    #[test]
    fn header_text_decodes_utf8_and_survives_garbage() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-device-model",
            HeaderValue::from_bytes("Пиксель 8".as_bytes()).unwrap(),
        );
        assert_eq!(
            header_text(&headers, "x-device-model", 64).as_deref(),
            Some("Пиксель 8")
        );

        headers.insert("x-device-model", HeaderValue::from_bytes(b"\xff\xfe").unwrap());
        assert!(
            header_text(&headers, "x-device-model", 64).is_some(),
            "мусорные байты не должны ронять разбор"
        );
    }

    #[test]
    fn device_signal_reads_all_five_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-hwid", HeaderValue::from_static("hw-1"));
        headers.insert("x-device-os", HeaderValue::from_static("Android"));
        headers.insert("x-ver-os", HeaderValue::from_static("14"));
        headers.insert("x-device-model", HeaderValue::from_static("Pixel 8"));
        headers.insert(header::USER_AGENT, HeaderValue::from_static("Happ/1.0.5"));

        let signal = device_signal(&headers);

        assert_eq!(signal.hardware_id(), Some("hw-1"));
        assert_eq!(signal.os_name(), Some("Android"));
        assert_eq!(signal.os_version(), Some("14"));
        assert_eq!(signal.device_model(), Some("Pixel 8"));
        assert_eq!(signal.user_agent(), Some("Happ/1.0.5"));
    }

    #[test]
    fn device_signal_without_headers_is_empty() {
        let signal = device_signal(&HeaderMap::new());

        assert_eq!(signal.hardware_id(), None);
        assert_eq!(signal.os_name(), None);
        assert_eq!(signal.os_version(), None);
        assert_eq!(signal.device_model(), None);
        assert_eq!(signal.user_agent(), None);
    }
}