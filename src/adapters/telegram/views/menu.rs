use crate::adapters::telegram::{MessageView, UiText, build_menu_keyboard};
use chrono::{DateTime, FixedOffset, Utc};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

#[must_use]
pub fn build_start_message(ui: &UiText, can_trial: bool) -> MessageView {
    let text = ui.message.msg_start_message.clone();

    let mut rows = Vec::new();

    if can_trial {
        rows.push(vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_trial.clone(),
            "menu:trial",
        )]);
    } else {
        rows.push(vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_vpn.clone(),
            "menu:myvpn",
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]);

    MessageView {
        text,
        keyboard: InlineKeyboardMarkup::new(rows),
    }
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn build_trial_success_view(ui: &UiText, expires_at: DateTime<Utc>) -> MessageView {
    let msk_offset = FixedOffset::east_opt(3 * 3600)
        .expect("Hardcoded offset of +3 hours is mathematically always valid");

    let expires_at_msk = expires_at.with_timezone(&msk_offset);
    let date_str = expires_at_msk.format("%d.%m.%Y %H:%M").to_string();

    let text = ui.message.msg_trial_success_view.replace("{date}", &date_str);

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_vpn.clone(),
            "menu:myvpn",
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_main.clone(),
            "menu:main",
        )],
    ]);

    MessageView { text, keyboard }
}

#[must_use]
pub fn build_main_menu_view(ui: &UiText, can_trial: bool) -> MessageView {
    MessageView {
        text: ui.message.msg_main_menu_view.clone(),
        keyboard: build_menu_keyboard(ui, can_trial),
    }
}

#[must_use]
pub fn build_refresh_menu_view(ui: &UiText, can_trial: bool) -> MessageView {
    MessageView {
        text: ui.message.msg_refresh_menu_view.clone(),
        keyboard: build_menu_keyboard(ui, can_trial),
    }
}

#[must_use]
pub fn build_referral_view(ui: &UiText, bot_username: &str, referral_code: &str) -> MessageView {
    let link = format!("https://t.me/{bot_username}?start={referral_code}");
    let text = ui.message.msg_referral.replace("{link}", &link);
    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]]);
    MessageView { text, keyboard }
}

#[must_use]
pub fn build_help_view(ui: &UiText) -> MessageView {
    let text = ui.message.msg_help.clone();
    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]]);
    MessageView { text, keyboard }
}

#[must_use]
pub fn build_about_view(ui: &UiText) -> MessageView {
    let text = ui.message.msg_about.clone();
    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]]);
    MessageView { text, keyboard }
}

#[must_use]
pub fn build_vpn_issued_view(ui: &UiText, link: &str) -> MessageView {
    let text = ui.message.msg_vpn_issued.replace("{link}", link);
    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]]);
    MessageView { text, keyboard }
}
