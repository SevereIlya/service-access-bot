mod menu;
mod profile;
mod purchase;

use chrono::FixedOffset;
pub use menu::*;
pub use profile::*;
pub use purchase::*;

use crate::adapters::telegram::UiText;
use crate::adapters::telegram::alerts::{AlertEvent, DEDUP_COOLDOWN};
use crate::application::AppError;
use crate::domain::{DomainError, SubscriptionError, UserError, subscription::SubscriptionPlan};
use std::collections::HashSet;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::utils::html::escape;

/// Лимит Telegram 4096 символов; оставляем запас под обрезку.
const MAX_ALERT_LEN: usize = 4000;

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

#[must_use]
pub(crate) fn build_alert_view(ui: &UiText, alert: &AlertEvent, suppressed: usize) -> String {
    let mut blocks: Vec<String> = Vec::with_capacity(4);

    if suppressed > 0 {
        let minutes = (DEDUP_COOLDOWN.as_secs() / 60).to_string();
        let count = suppressed.to_string();
        blocks.push(
            ui.alert
                .alert_suppressed
                .replace("{suppressed}", &count)
                .replace("{minutes}", &minutes),
        );
    }

    let mut head = ui.alert.alert_what.replace("{message}", &escape(&alert.message));
    if let (Some(file), Some(line)) = (&alert.file, alert.line) {
        let place = format!("{file}:{line}");
        let where_line = ui.alert.alert_where.replace("{where}", &escape(&place));
        head.push('\n');
        head.push_str(&where_line);
    }
    let module_line = ui.alert.alert_module.replace("{module}", &escape(&alert.target));
    head.push('\n');
    head.push_str(&module_line);
    blocks.push(head);

    let mut context: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (name, value) in alert.fields.iter().chain(alert.span_fields.iter()) {
        if seen.insert(name.clone()) {
            context.push((escape(name), escape(value)));
        }
    }
    if !context.is_empty() {
        let lines: Vec<String> = context
            .iter()
            .map(|(name, value)| {
                ui.alert.alert_context_line.replace("{name}", name).replace("{value}", value)
            })
            .collect();
        blocks.push(format!(
            "{}\n{}",
            ui.alert.alert_context_header,
            lines.join("\n")
        ));
    }

    let msk = FixedOffset::east_opt(3 * 3600)
        .expect("Hardcoded offset of +3 hours is mathematically always valid");
    let date = alert.timestamp.with_timezone(&msk).format("%d.%m.%Y %H:%M").to_string();
    blocks.push(ui.alert.alert_when.replace("{date}", &date));

    let mut text = format!("{}\n\n{}", ui.alert.alert_title, blocks.join("\n\n"));

    if text.chars().count() > MAX_ALERT_LEN {
        let mut cut: String = text.chars().take(MAX_ALERT_LEN - 1).collect();
        cut.push('…');
        text = cut;
    }
    text
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
