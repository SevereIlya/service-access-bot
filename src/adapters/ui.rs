use anyhow::Context;
use serde::Deserialize;
use std::fs::read_to_string;

#[derive(Debug, Clone, Deserialize)]
pub struct UiText {
    pub message: MsgText,
    pub stub: StubText,
    pub button: ButtonText,
    pub alert: AlertText,
    pub error: ErrorText,
}
#[derive(Debug, Clone, Deserialize)]
pub struct MsgText {
    pub msg_start_message: String,
    pub msg_trial_success_view: String,
    pub msg_main_menu_view: String,
    pub msg_refresh_menu_view: String,
    pub msg_unknown_command: String,
    pub msg_subscription_expiring: String,
    pub msg_subscription_expired: String,
    pub msg_vpn_issued: String,
    pub msg_profile: String,
    pub msg_referral: String,
    pub msg_help: String,
    pub msg_about: String,
    pub msg_tariffs_select: String,
    pub msg_purchase_confirm: String,
    pub msg_purchase_insufficient: String,
    pub msg_top_up_link: String,
    pub msg_purchase_success: String,
    pub msg_balance_topped_up: String,
    pub msg_new_device_detected: String,
    pub msg_device_setup_code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StubText {
    pub stub_no_subscription: String,
    pub stub_needs_hwid: String,
    pub stub_awaiting_setup: String,
    pub stub_not_registered: String,
    pub stub_revoked: String,
    pub stub_unsupported_client: String,
    pub stub_limit_exceeded: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ButtonText {
    pub btn_menu_trial: String,
    pub btn_menu_main: String,
    pub btn_menu_vpn: String,
    pub btn_menu_profile: String,
    pub btn_menu_tariffs: String,
    pub btn_menu_referral: String,
    pub btn_menu_about: String,
    pub btn_menu_help: String,
    pub btn_menu_down: String,
    pub btn_tariff_week_1: String,
    pub btn_tariff_month_1: String,
    pub btn_tariff_month_3: String,
    pub btn_tariff_month_6: String,
    pub btn_pay: String,
    pub btn_back: String,
    pub btn_top_up_amount: String,
    pub btn_open_payment: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertText {
    pub alert_title: String,
    pub alert_suppressed: String,
    pub alert_what: String,
    pub alert_where: String,
    pub alert_module: String,
    pub alert_context_header: String,
    pub alert_context_line: String,
    pub alert_when: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorText {
    pub err_user_not_found: String,
    pub err_trial_used: String,
    pub err_has_sub: String,
    pub err_system_failure: String,
    pub err_internal: String,
    pub err_no_active_subscription: String,
}

impl UiText {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = read_to_string(path).context("Не удалось прочитать файл локализации")?;
        let ui = toml::from_str(&content).context("Ошибка парсинга TOML файла локализации")?;
        Ok(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (поле, текст, плейсхолдеры)
    type Case<'a> = (&'a str, &'a str, &'a [&'a str]);

    #[rustfmt::skip]
    fn verify_placeholders(ui: &UiText, file_path: &str) {
        let (m, b, a, s) = (&ui.message, &ui.button, &ui.alert, &ui.stub);
        let cases: &[Case<'_>] = &[
            
            // message
            ("msg_trial_success_view", m.msg_trial_success_view.as_str(), &["{date}"]),
            ("msg_subscription_expiring", m.msg_subscription_expiring.as_str(), &["{date}"]),
            ("msg_vpn_issued", m.msg_vpn_issued.as_str(), &["{link}"]),
            ("msg_profile", m.msg_profile.as_str(), &["{telegram_id}", "{referral_code}", "{subscription}"]),
            ("msg_referral", m.msg_referral.as_str(), &["{link}"]),
            ("msg_purchase_confirm", m.msg_purchase_confirm.as_str(), &["{plan}", "{devices}", "{price}"]),
            ("msg_purchase_insufficient", m.msg_purchase_insufficient.as_str(), &["{plan}", "{devices}", "{price}", "{shortfall}"]),
            ("msg_top_up_link", m.msg_top_up_link.as_str(), &["{amount}"]),
            ("msg_purchase_success", m.msg_purchase_success.as_str(), &["{plan}", "{devices}", "{date}"]),
            ("msg_balance_topped_up", m.msg_balance_topped_up.as_str(), &["{amount}", "{balance}"]),
            ("msg_new_device_detected", m.msg_new_device_detected.as_str(), &["{device}"]),
            ("msg_device_setup_code", m.msg_device_setup_code.as_str(), &["{device}", "{code}"]),

            // stub
            ("stub_awaiting_setup", s.stub_awaiting_setup.as_str(), &["{code}"]),
            ("stub_limit_exceeded", s.stub_limit_exceeded.as_str(), &["{current}", "{limit}"]),

            // button
            ("btn_top_up_amount", b.btn_top_up_amount.as_str(), &["{amount}"]),
            
            // alert
            ("alert_suppressed", a.alert_suppressed.as_str(), &["{suppressed}", "{minutes}"]),
            ("alert_what", a.alert_what.as_str(), &["{message}"]),
            ("alert_where", a.alert_where.as_str(), &["{where}"]),
            ("alert_module", a.alert_module.as_str(), &["{module}"]),
            ("alert_context_line", a.alert_context_line.as_str(), &["{name}", "{value}"]),
            ("alert_when", a.alert_when.as_str(), &["{date}"]),
        ];

        for &(field, text, placeholders) in cases {
            for &placeholder in placeholders {
                assert!(
                    text.contains(placeholder),
                    "В файле '{file_path}' в поле '{field}' пропущен плейсхолдер '{placeholder}'"
                );
            }
        }
    }

    #[test]
    fn locales_keep_their_placeholders() {
        for path in ["locales/ru.toml", "locales/en.example.toml"] {
            if !std::path::Path::new(path).exists() {
                eprintln!("пропускаю '{path}': файла нет");
                continue;
            }
            let ui = UiText::load(path).unwrap_or_else(|e| {
                panic!("Не смогли распарсить файл локали '{path}'. Ошибка: {e}")
            });
            verify_placeholders(&ui, path);
        }
    }
}
