mod menu;
mod profile;
mod purchase;

pub use menu::*;
pub use profile::*;
pub use purchase::*;

use crate::adapters::telegram::UiText;
use crate::application::AppError;
use crate::domain::{DomainError, SubscriptionError, UserError, subscription::SubscriptionPlan};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub(crate) fn plan_label(ui: &UiText, plan: SubscriptionPlan) -> String {
    match plan {
        SubscriptionPlan::Week1 => ui.button.btn_tariff_week_1.clone(),
        SubscriptionPlan::Month1 => ui.button.btn_tariff_month_1.clone(),
        SubscriptionPlan::Month3 => ui.button.btn_tariff_month_3.clone(),
        SubscriptionPlan::Month6 => ui.button.btn_tariff_month_6.clone(),
        SubscriptionPlan::Trial => "Пробный период".to_string(),
    }
}

pub(crate) fn build_menu_keyboard(ui: &UiText, can_trial: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::with_capacity(6);

    if can_trial {
        rows.push(vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_trial.clone(),
            "menu:trial",
        )]);
    }

    rows.extend([
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_vpn.clone(),
            "menu:myvpn",
        )],
        vec![
            InlineKeyboardButton::callback(ui.button.btn_menu_profile.clone(), "menu:profile"),
            InlineKeyboardButton::callback(ui.button.btn_menu_tariffs.clone(), "menu:tariffs"),
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
            ui.button.btn_menu_about.clone(),
            "menu:about",
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_down.clone(),
            "menu:down",
        )],
    ]);

    InlineKeyboardMarkup::new(rows)
}

#[rustfmt::skip]
#[must_use]
pub(crate) fn message_error(ui: &UiText, err: &AppError) -> String {
    match err {
        AppError::Domain(domain_err) => match domain_err {
            DomainError::User(UserError::NotFound) => ui.error.err_user_not_found.clone(),
            DomainError::User(UserError::TrialAlreadyUsed) => ui.error.err_trial_used.clone(),
            DomainError::Subscription(SubscriptionError::AlreadyHasActive) => ui.error.err_has_sub.clone(),
            DomainError::Subscription(SubscriptionError::NoActiveSubscription) => ui.error.err_no_active_subscription.clone(),
            DomainError::SystemFailure(_) => ui.error.err_system_failure.clone(),
            _ => ui.error.err_internal.clone(),
        },
        _ => ui.error.err_internal.clone(),
    }
}
