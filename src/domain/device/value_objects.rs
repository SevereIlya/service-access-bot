use crate::domain::error::{DeviceError, DomainError};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(i64);

impl DeviceId {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Код настройки устройства для Clash Verge.
///
/// Код представляет собой UUID v4 в форме `Uuid::simple()`: ровно 32 hex-символа без дефисов.
/// Пользователь указывает этот код в User-Agent клиента,
/// после чего код используется для идентификации устройства при следующем запросе подписки.
///
/// Значение создаётся через [`Self::generate`] или [`Self::parse`],
/// поэтому `DeviceSetupCode` всегда соответствует ожидаемому формату.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceSetupCode(String);

impl DeviceSetupCode {
    /// Количество символов
    const LEN: usize = 32;

    /// Генерирует новый код
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    /// Строка обрезается по краям от пробелов, после чего проверяется на длину и наличие hex-символов.
    /// Результат нормализуется к нижнему регистру.
    ///
    /// Возвращает [`None`], если строка не соответствует формату кода.
    /// Это ожидаемый результат для обычного User-Agent, не являющийся кодом.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let is_code = raw.len() == Self::LEN && raw.bytes().all(|b| b.is_ascii_hexdigit());
        is_code.then(|| Self(raw.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Display for DeviceSetupCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Уникальный отпечаток устройства внутри пользователя.
///
/// Отпечаток содержит префикс, определяющий источник идентификатора: `hwid:{id}` или `code:{код}`.
/// Благодаря этому HWID и код находятся в разных пространствах даже при совпадении самих значений.
///
/// Создание отпечатка из исходных идентификаторов выполняется через [`Self::from_hwid`] и [`Self::from_code`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceFingerprint(String);

impl DeviceFingerprint {
    /// Восстанавливает отпечаток из сохранённого значения.
    ///
    /// Значение не проверяется и считается уже корректным.
    /// Метод предназначен для восстановления доменного объекта из БД.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn from_hwid(hwid: &str) -> Self {
        Self(format!("hwid:{hwid}"))
    }

    #[must_use]
    pub fn from_code(code: &DeviceSetupCode) -> Self {
        Self(format!("code:{code}"))
    }

    /// Достаёт код, если это фингерпринт вида `code:{code}`.
    /// Возвращает [`None`] если это фингерпринт другого вида
    #[must_use]
    pub fn as_setup_code(&self) -> Option<DeviceSetupCode> {
        self.0.strip_prefix("code:").and_then(DeviceSetupCode::parse)
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Display for DeviceFingerprint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Определяет, как бот обрабатывает подписки от клиента.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClientSupport {
    /// Клиент поддерживается.
    Managed,

    /// Клиент не поддерживается, но может читать YAML.
    NoticeOnly,

    /// Клиент не поддерживает YAML.
    Rejected,
}

/// VPN-приложение, определённое по User-Agent.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AppClient {
    ClashVerge,
    ClashMi,
    ClashMeta,
    Happ,
    Incy,
}

impl AppClient {
    #[must_use]
    pub fn from_user_agent(user_agent: &str) -> Option<Self> {
        let ua = user_agent.trim().to_ascii_lowercase();
        match () {
            () if ua.contains("clash-verge/") => Some(Self::ClashVerge),
            () if ua.contains("clashmeta/") && ua.contains("mihomo/") => Some(Self::ClashMi),
            () if ua.contains("clashmetaforandroid/") => Some(Self::ClashMeta),
            () if ua.contains("happ/") => Some(Self::Happ),
            () if ua.contains("incy/") => Some(Self::Incy),
            () => None,
        }
    }

    /// Человекочитаемое название для имени устройства.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::ClashVerge => "Clash Verge",
            Self::ClashMi => "Clash Mi",
            Self::ClashMeta => "Clash Meta",
            Self::Happ => "Happ",
            Self::Incy => "INCY",
        }
    }

    /// Поддержка приложений
    #[must_use]
    pub const fn support(self) -> ClientSupport {
        match self {
            Self::ClashVerge | Self::ClashMi => ClientSupport::Managed,
            Self::ClashMeta => ClientSupport::NoticeOnly,
            Self::Happ | Self::Incy => ClientSupport::Rejected,
        }
    }
}

/// Результат определения идентичности устройства по [`DeviceSignal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceIdentity {
    /// Клиент передал HWID
    Hwid(String),

    /// User-Agent содержит выданный код настройки.
    IssuedCode(DeviceSetupCode),

    /// Некорректный UA-код
    MalformedCode,

    /// Известное приложение не предоставило надёжного идентификатора.
    Unconfigured(AppClient),

    /// Клиент не распознан, идентификатор отсутствует.
    Unknown,
}

impl DeviceIdentity {
    /// Фингерпринт для идентификации устройства.
    ///
    /// HWID и выданный код преобразуются в [`DeviceFingerprint`].
    /// Для [`Self::Unconfigured`] и [`Self::Unknown`] возвращается [`None`], поскольку у них отсутствует идентификатор.
    #[must_use]
    pub fn fingerprint(&self) -> Option<DeviceFingerprint> {
        match self {
            Self::Hwid(hwid) => Some(DeviceFingerprint::from_hwid(hwid)),
            Self::IssuedCode(code) => Some(DeviceFingerprint::from_code(code)),
            Self::Unconfigured(_) | Self::MalformedCode | Self::Unknown => None,
        }
    }
}

/// Данные об устройстве, извлечённые из HTTP-запроса подписки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSignal {
    hardware_id: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
    device_model: Option<String>,
    user_agent: Option<String>,
}

impl DeviceSignal {
    #[must_use]
    pub const fn new(
        hardware_id: Option<String>,
        os_name: Option<String>,
        os_version: Option<String>,
        device_model: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            hardware_id,
            os_name,
            os_version,
            device_model,
            user_agent,
        }
    }

    #[must_use]
    pub fn hardware_id(&self) -> Option<&str> {
        self.hardware_id.as_deref()
    }

    #[must_use]
    pub fn os_name(&self) -> Option<&str> {
        self.os_name.as_deref()
    }

    #[must_use]
    pub fn os_version(&self) -> Option<&str> {
        self.os_version.as_deref()
    }

    #[must_use]
    pub fn device_model(&self) -> Option<&str> {
        self.device_model.as_deref()
    }

    #[must_use]
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    /// Определяет приложение/клиент по User-Agent.
    ///
    /// Используется как для нового сигнала, так и для сохранённого сигнала с исходным User-Agent.
    #[must_use]
    pub fn app_client(&self) -> Option<AppClient> {
        self.user_agent().and_then(AppClient::from_user_agent)
    }

    /// Определяет устройство по содержащимся в сигнале данным.
    ///
    /// Сначала определяется приложение и его поддержка ([`AppClient::support`]):
    /// для `Managed` используется HWID, если он задан,
    /// для `NoticeOnly` - [`DeviceIdentity::Unconfigured`],
    /// для `Rejected` - [`DeviceIdentity::Unknown`].
    ///
    /// Если приложение не распознано, проверяется `User-Agent` на наличие выданного кода настройки.
    /// Если код состоит только из hex-символов, но не распознаётся как корректный код, возвращается [`DeviceIdentity::MalformedCode`].
    ///
    /// Пустой или состоящий только из пробелов HWID игнорируется.
    #[must_use]
    pub fn identity(&self) -> DeviceIdentity {
        if let Some(app) = self.app_client() {
            return match app.support() {
                ClientSupport::Managed => self
                    .hardware_id()
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map_or(DeviceIdentity::Unconfigured(app), |id| {
                        DeviceIdentity::Hwid(id.to_owned())
                    }),
                ClientSupport::NoticeOnly => DeviceIdentity::Unconfigured(app),
                ClientSupport::Rejected => DeviceIdentity::Unknown,
            };
        }
        let Some(user_agent) = self.user_agent() else {
            return DeviceIdentity::Unknown;
        };
        if let Some(code) = DeviceSetupCode::parse(user_agent) {
            return DeviceIdentity::IssuedCode(code);
        }
        if !user_agent.is_empty() && user_agent.bytes().all(|b| b.is_ascii_hexdigit()) {
            return DeviceIdentity::MalformedCode;
        }
        DeviceIdentity::Unknown
    }
}

/// Состояние устройства в реестре.
///
/// `Pending` означает, что устройство ещё не подтвердилось.
/// `Active` означает, что устройство подтверждено.
/// `Revoked` означает, что устройство отключено, но его запись сохраняется в реестре.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DeviceStatus {
    Pending,
    Active,
    Revoked,
}

impl DeviceStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }

    /// Определяет, занимает ли устройство слот лимита устройств пользователя.
    ///
    /// `Pending` и `Active` учитываются в лимите, `Revoked` — нет.
    #[must_use]
    pub const fn counts_toward_limit(self) -> bool {
        matches!(self, Self::Pending | Self::Active)
    }
}

impl Display for DeviceStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DeviceStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            other => Err(DeviceError::InvalidStatus(other.to_owned()).into()),
        }
    }
}

/// Отображаемое пользователю имя устройства.
///
/// Значение очищается через [`Self::parse`] при создании и затем сохраняется в БД.
/// По умолчанию имя строится из модели устройства, названия приложения или операционной системы.
///
/// HTML-экранирование здесь намеренно не выполняется: это ответственность слоя, который выводит имя в конкретном формате.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceName(String);

impl DeviceName {
    const MAX_CHARS: usize = 64;
    const FALLBACK: &'static str = "Устройство";

    /// Очищает и разбирает исходную строку как имя устройства.
    ///
    /// Удаляются управляющие символы, затем результат ограничивается [`Self::MAX_CHARS`] символами и обрезается по краям от пробелов.
    ///
    /// Возвращает [`None`], если после очистки строка оказалась пустой.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let cleaned: String =
            raw.chars().filter(|c| !c.is_control()).take(Self::MAX_CHARS).collect();
        let trimmed = cleaned.trim();
        (!trimmed.is_empty()).then(|| Self(trimmed.to_owned()))
    }

    /// Восстанавливает имя из сохранённого значения.
    #[must_use]
    pub const fn restore(value: String) -> Self {
        Self(value)
    }

    /// Формирует имя для нового устройства на основе данных [`DeviceSignal`].
    ///
    /// Источники проверяются в следующем порядке:
    ///
    /// 1. модель устройства;
    /// 2. название приложения с порядковым номером;
    /// 3. название операционной системы;
    /// 4. [`Self::FALLBACK`].
    ///
    /// `same_app_count` используется для вычисления следующего порядкового номера устройства данного приложения.
    #[must_use]
    pub fn for_new_device(signal: &DeviceSignal, same_app_count: usize) -> Self {
        signal
            .device_model()
            .and_then(Self::parse)
            .or_else(|| signal.app_client().map(|app| Self::numbered(app, same_app_count + 1)))
            .or_else(|| signal.os_name().and_then(Self::parse))
            .unwrap_or_else(|| Self(Self::FALLBACK.to_owned()))
    }

    fn numbered(app: AppClient, ordinal: usize) -> Self {
        Self(format!("{} {ordinal:02}", app.title()))
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Display for DeviceName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "0123456789abcdef0123456789abcdef";
    const CLASH_VERGE_UA: &str = "clash-verge/v2.4.3";
    const CLASHMI_UA: &str = "ClashMeta/1.19.30; mihomo/1.19.30";
    const CLASHMETA_ANDROID_UA: &str = "ClashMetaForAndroid/2.8.9";
    const HAPP_UA: &str = "Happ/1.0.5";
    const INCY_UA: &str = "Incy/2.1.0";

    fn signal(
        hwid: Option<&str>,
        os: Option<&str>,
        model: Option<&str>,
        user_agent: Option<&str>,
    ) -> DeviceSignal {
        DeviceSignal::new(
            hwid.map(String::from),
            os.map(String::from),
            None,
            model.map(String::from),
            user_agent.map(String::from),
        )
    }

    // --- DeviceId ---
    #[test]
    fn device_id_inner_and_display() {
        let id = DeviceId::new(42);
        assert_eq!(id.inner(), 42);
        assert_eq!(id.to_string(), "42");
    }

    // --- DeviceSetupCode ---

    #[test]
    fn generated_code_is_valid_and_unique() {
        let a = DeviceSetupCode::generate();
        let b = DeviceSetupCode::generate();

        assert_eq!(a.inner().len(), 32);
        assert_eq!(DeviceSetupCode::parse(a.inner()), Some(a.clone()));
        assert_ne!(a, b);
    }

    #[test]
    fn code_parse_normalizes_case_and_whitespace() {
        let raw = format!("  {}  ", CODE.to_uppercase());

        let code = DeviceSetupCode::parse(&raw).unwrap();

        assert_eq!(code.inner(), CODE);
    }

    #[test]
    fn code_parse_rejects_things_that_are_not_codes() {
        assert!(DeviceSetupCode::parse("").is_none());
        assert!(DeviceSetupCode::parse(&CODE[..31]).is_none(), "короче 32");
        assert!(
            DeviceSetupCode::parse(&format!("{CODE}0")).is_none(),
            "длиннее 32"
        );
        assert!(DeviceSetupCode::parse(&"g".repeat(32)).is_none(), "не hex");
        assert!(
            DeviceSetupCode::parse("0123456789abcdef-0123456789abcdef-0123").is_none(),
            "uuid с дефисами — не наш формат"
        );
        assert!(DeviceSetupCode::parse(CLASH_VERGE_UA).is_none());
    }

    // --- DeviceFingerprint ---

    #[test]
    fn fingerprint_prefixes_keep_namespaces_apart() {
        let code = DeviceSetupCode::parse(CODE).unwrap();

        assert_eq!(DeviceFingerprint::from_hwid("abc").inner(), "hwid:abc");
        assert_eq!(
            DeviceFingerprint::from_code(&code).inner(),
            format!("code:{CODE}")
        );
        assert_ne!(
            DeviceFingerprint::from_hwid(CODE),
            DeviceFingerprint::from_code(&code)
        );
    }

    #[test]
    fn fingerprint_returns_setup_code_only_for_code_kind() {
        let code = DeviceSetupCode::parse(CODE).unwrap();

        assert_eq!(
            DeviceFingerprint::from_code(&code).as_setup_code(),
            Some(code)
        );
        assert!(DeviceFingerprint::from_hwid(CODE).as_setup_code().is_none());
        assert!(DeviceFingerprint::new("code:not-a-code".into()).as_setup_code().is_none());
    }

    // --- AppClient ---

    #[test]
    fn app_is_detected_by_known_prefixes_case_insensitive() {
        assert_eq!(
            AppClient::from_user_agent(CLASH_VERGE_UA),
            Some(AppClient::ClashVerge)
        );
        assert_eq!(
            AppClient::from_user_agent("Clash-Verge/V2.0.0"),
            Some(AppClient::ClashVerge)
        );
        assert_eq!(
            AppClient::from_user_agent(CLASHMI_UA),
            Some(AppClient::ClashMi)
        );
        assert_eq!(
            AppClient::from_user_agent(CLASHMETA_ANDROID_UA),
            Some(AppClient::ClashMeta)
        );
        assert_eq!(AppClient::from_user_agent(HAPP_UA), Some(AppClient::Happ));
        assert_eq!(AppClient::from_user_agent(INCY_UA), Some(AppClient::Incy));
    }

    #[test]
    fn app_is_none_for_unknown_clients() {
        assert!(AppClient::from_user_agent("Hiddify/2.0.5 (android)").is_none());
        assert!(AppClient::from_user_agent("Mozilla/5.0 (Windows NT 10.0)").is_none());
        assert!(
            AppClient::from_user_agent("clash-verge").is_none(),
            "без слэша"
        );
        assert!(AppClient::from_user_agent("").is_none());
    }

    #[test]
    fn app_titles_are_correct() {
        assert_eq!(AppClient::ClashVerge.title(), "Clash Verge");
        assert_eq!(AppClient::ClashMi.title(), "Clash Mi");
        assert_eq!(AppClient::ClashMeta.title(), "Clash Meta");
        assert_eq!(AppClient::Happ.title(), "Happ");
        assert_eq!(AppClient::Incy.title(), "INCY");
    }

    // --- DeviceSignal::identity ---

    #[test]
    fn identity_prefers_hwid_over_everything() {
        let s = signal(Some("hw-1"), None, None, Some(CLASH_VERGE_UA));

        assert_eq!(s.identity(), DeviceIdentity::Hwid("hw-1".into()));
    }

    #[test]
    fn identity_trims_hwid_and_ignores_blank_one() {
        let padded = signal(Some("  hw-1 "), None, None, Some(CLASHMI_UA));
        let blank = signal(Some("   "), None, None, Some(CLASH_VERGE_UA));

        assert_eq!(padded.identity(), DeviceIdentity::Hwid("hw-1".into()));
        assert_eq!(
            blank.identity(),
            DeviceIdentity::Unconfigured(AppClient::ClashVerge)
        );
    }

    #[test]
    fn identity_recognizes_issued_code_in_user_agent() {
        let s = signal(None, None, None, Some(CODE));

        assert_eq!(
            s.identity(),
            DeviceIdentity::IssuedCode(DeviceSetupCode::parse(CODE).unwrap())
        );
    }

    #[test]
    fn identity_of_default_clients_is_unconfigured() {
        assert_eq!(
            signal(None, None, None, Some(CLASH_VERGE_UA)).identity(),
            DeviceIdentity::Unconfigured(AppClient::ClashVerge)
        );
        assert_eq!(
            signal(None, None, None, Some(CLASHMI_UA)).identity(),
            DeviceIdentity::Unconfigured(AppClient::ClashMi)
        );
    }

    #[test]
    fn identity_of_unrecognized_client_is_unknown() {
        assert_eq!(
            signal(None, None, None, Some("curl/8.0")).identity(),
            DeviceIdentity::Unknown
        );
        assert_eq!(
            signal(None, None, None, None).identity(),
            DeviceIdentity::Unknown
        );
    }

    // --- DeviceIdentity::fingerprint ---

    #[test]
    fn only_hwid_and_code_identities_have_fingerprint() {
        let code = DeviceSetupCode::parse(CODE).unwrap();

        assert_eq!(
            DeviceIdentity::Hwid("hw-1".into()).fingerprint(),
            Some(DeviceFingerprint::from_hwid("hw-1"))
        );
        assert_eq!(
            DeviceIdentity::IssuedCode(code.clone()).fingerprint(),
            Some(DeviceFingerprint::from_code(&code))
        );
        assert_eq!(DeviceIdentity::MalformedCode.fingerprint(), None);
        assert!(DeviceIdentity::Unconfigured(AppClient::ClashVerge).fingerprint().is_none());
        assert!(DeviceIdentity::Unknown.fingerprint().is_none());
    }

    #[test]
    fn identity_accepts_hwid_only_from_supported_apps() {
        assert_eq!(
            signal(Some("hw-1"), None, None, Some(CLASHMI_UA)).identity(),
            DeviceIdentity::Hwid("hw-1".into())
        );
        assert_eq!(
            signal(Some(" hw-1  "), None, None, Some(CLASHMI_UA)).identity(),
            DeviceIdentity::Hwid("hw-1".into())
        );
        assert_eq!(
            signal(Some("hw-1"), None, None, Some("ClashMetaForAndroid/2.11.32.Meta")).identity(),
            DeviceIdentity::Unconfigured(AppClient::ClashMeta)
        );
        assert_eq!(
            signal(Some("   "), None, None, Some(CLASHMI_UA)).identity(),
            DeviceIdentity::Unconfigured(AppClient::ClashMi)
        );
    }

    #[test]
    fn identity_rejects_clients_that_cannot_read_our_yaml() {
        let rejected = [
            "Happ/4.7.1/Android/0123456789abcdef0123456789abcdef",
            "INCY/3.7.0/android Dalvik/2.1.0",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
        ];
        for ua in rejected {
            assert_eq!(
                signal(Some("hw-1"), None, None, Some(ua)).identity(),
                DeviceIdentity::Unknown,
                "{ua}"
            );
        }
        assert_eq!(signal(Some("hw-1"), None, None, None).identity(), DeviceIdentity::Unknown);
    }

    #[test]
    fn identity_flags_hex_only_user_agent_as_malformed_code() {
        let too_long = format!("{CODE}a");
        let upper_partial = CODE.to_uppercase()[..20].to_owned();

        for ua in [&CODE[..31], too_long.as_str(), upper_partial.as_str()] {
            assert_eq!(
                signal(None, None, None, Some(ua)).identity(),
                DeviceIdentity::MalformedCode,
                "{ua}"
            );
        }
        assert_eq!(signal(None, None, None, Some("my custom ua")).identity(), DeviceIdentity::Unknown);
        assert_eq!(signal(None, None, None, Some("   ")).identity(), DeviceIdentity::Unknown);
    }

    #[test]
    fn app_support_policy() {
        assert_eq!(AppClient::ClashVerge.support(), ClientSupport::Managed);
        assert_eq!(AppClient::ClashMi.support(), ClientSupport::Managed);
        assert_eq!(AppClient::ClashMeta.support(), ClientSupport::NoticeOnly);
        assert_eq!(AppClient::Happ.support(), ClientSupport::Rejected);
        assert_eq!(AppClient::Incy.support(), ClientSupport::Rejected);
    }

    // --- DeviceStatus ---

    #[test]
    fn status_roundtrips_through_string() {
        for status in [
            DeviceStatus::Pending,
            DeviceStatus::Active,
            DeviceStatus::Revoked,
        ] {
            assert_eq!(status.as_str().parse::<DeviceStatus>().unwrap(), status);
            assert_eq!(status.to_string(), status.as_str());
        }
    }

    #[test]
    fn status_rejects_unknown_string() {
        let err = "banned".parse::<DeviceStatus>().unwrap_err();

        assert!(matches!(
            err,
            DomainError::Device(DeviceError::InvalidStatus(s)) if s == "banned"
        ));
    }

    #[test]
    fn revoked_device_does_not_count_toward_limit() {
        assert!(DeviceStatus::Pending.counts_toward_limit());
        assert!(DeviceStatus::Active.counts_toward_limit());
        assert!(!DeviceStatus::Revoked.counts_toward_limit());
    }

    // --- DeviceName ---

    #[test]
    fn name_parse_strips_control_chars_and_trims() {
        let name = DeviceName::parse("  Pixel\n 8\t ").unwrap();

        assert_eq!(name.inner(), "Pixel 8");
    }

    #[test]
    fn name_parse_rejects_empty_result() {
        assert!(DeviceName::parse("").is_none());
        assert!(DeviceName::parse("   ").is_none());
        assert!(DeviceName::parse("\n\t\r").is_none());
    }

    #[test]
    fn name_parse_truncates_by_chars_not_bytes() {
        let name = DeviceName::parse(&"я".repeat(100)).unwrap();

        assert_eq!(name.inner().chars().count(), 64);
    }

    #[test]
    fn name_parse_does_not_escape_html() {
        let name = DeviceName::parse("<b>x</b>").unwrap();

        assert_eq!(name.inner(), "<b>x</b>");
    }

    #[test]
    fn new_device_name_prefers_device_model() {
        let s = signal(
            Some("hw-1"),
            Some("Android"),
            Some("Pixel 8"),
            Some(CLASHMI_UA),
        );

        assert_eq!(DeviceName::for_new_device(&s, 5).inner(), "Pixel 8");
    }

    #[test]
    fn new_device_name_is_numbered_by_app() {
        let s = signal(None, None, None, Some(CLASH_VERGE_UA));

        assert_eq!(DeviceName::for_new_device(&s, 0).inner(), "Clash Verge 01");
        assert_eq!(DeviceName::for_new_device(&s, 11).inner(), "Clash Verge 12");
    }

    #[test]
    fn blank_model_falls_through_to_app_name() {
        let s = signal(Some("hw-1"), None, Some("  "), Some(CLASHMI_UA));

        assert_eq!(DeviceName::for_new_device(&s, 0).inner(), "Clash Mi 01");
    }

    #[test]
    fn new_device_name_falls_back_to_os_then_default() {
        let with_os = signal(Some("hw-1"), Some("Android"), None, None);
        let nothing = signal(Some("hw-2"), None, None, None);

        assert_eq!(DeviceName::for_new_device(&with_os, 0).inner(), "Android");
        assert_eq!(
            DeviceName::for_new_device(&nothing, 0).inner(),
            "Устройство"
        );
    }
}
