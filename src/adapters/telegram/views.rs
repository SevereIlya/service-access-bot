use crate::adapters::telegram::handlers::MessageView;
use crate::adapters::telegram::ui::UiText;
use crate::application::error::AppError;
use crate::domain::error::DomainError;
use crate::domain::error::SubscriptionError::AlreadyHasActive;
use crate::domain::error::UserError::{NotFound, TrialAlreadyUsed};
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
            ui.button.btn_menu_router.clone(),
            "menu:router",
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
            ui.button.btn_menu_router.clone(),
            "menu:router",
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

// ============================================================================================== //

fn build_menu_keyboard(ui: &UiText, can_trial: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::with_capacity(6);

    if can_trial {
        rows.push(vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_trial.clone(),
            "menu:trial",
        )]);
    }

    rows.extend([
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_router.clone(),
            "menu:router",
        )],
        vec![
            InlineKeyboardButton::callback(
                ui.button.btn_menu_profile.clone(),
                "menu:profile",
            ),
            InlineKeyboardButton::callback(
                ui.button.btn_menu_tariffs.clone(),
                "menu:tariffs",
            ),
        ],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_referral.clone(),
            "menu:referral",
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_help.clone(),
            "menu:help",
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_down.clone(),
            "menu:down",
        )],
    ]);

    InlineKeyboardMarkup::new(rows)
}

// ============================================================================================== //

#[must_use]
pub fn message_error(ui: &UiText, err: &AppError) -> String {
    match err {
        AppError::Domain(domain_err) => match domain_err {
            DomainError::User(NotFound) => ui.error.err_user_not_found.clone(),
            DomainError::User(TrialAlreadyUsed) => ui.error.err_trial_used.clone(),
            DomainError::Subscription(AlreadyHasActive) => ui.error.err_has_sub.clone(),
            DomainError::SystemFailure(_) => ui.error.err_system_failure.clone(),
            _ => ui.error.err_internal.clone(),
        },
        _ => ui.error.err_internal.clone(),
    }
}
