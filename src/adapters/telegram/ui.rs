use anyhow::Context;
use serde::Deserialize;
use std::fs::read_to_string;

#[derive(Debug, Clone, Deserialize)]
pub struct UiText {
    pub message: MsgText,
    pub button: ButtonText,
    pub error: ErrorText,
}
#[derive(Debug, Clone, Deserialize)]
pub struct MsgText {
    pub msg_start_message: String,
    pub msg_trial_success_view: String,
    pub msg_main_menu_view: String,
    pub msg_refresh_menu_view: String,
    pub msg_unknown_command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ButtonText {
    pub btn_menu_trial: String,
    pub btn_menu_main: String,
    pub btn_menu_router: String,
    pub btn_menu_profile: String,
    pub btn_menu_tariffs: String,
    pub btn_menu_referral: String,
    pub btn_menu_help: String,
    pub btn_menu_down: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorText {
    pub err_user_not_found: String,
    pub err_trial_used: String,
    pub err_has_sub: String,
    pub err_system_failure: String,
    pub err_internal: String,
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
