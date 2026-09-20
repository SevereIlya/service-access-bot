use anyhow::Context;
use serde::Deserialize;
use std::fs::read_to_string;

#[derive(Debug, Clone, Deserialize)]
pub struct UiText {
    pub message: MsgText,
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
        let content =
            read_to_string(path).context("Не удалось прочитать файл локализации")?;
        let ui =
            toml::from_str(&content).context("Ошибка парсинга TOML файла локализации")?;
        Ok(ui)
    }
}
