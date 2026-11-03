use crate::adapters::telegram::{MessageView, UiText, format_rub_exact, plan_label};
use crate::application::ProfileSnapshot;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

#[must_use]
pub fn build_profile_view(ui: &UiText, snapshot: &ProfileSnapshot) -> MessageView {
    let subscription_line = snapshot.subscription.as_ref().map_or_else(
        || "Нет активной подписки".to_string(),
        |sub| {
            format!(
                "Тариф: {}\nДействует до: {} (осталось {} дн.)\nУстройства: {} из {}",
                plan_label(ui, sub.plan),
                sub.expires_at.format("%d.%m.%Y"),
                sub.days_left.max(0),
                sub.devices.devices_used,
                sub.devices.devices_limit,
            )
        },
    );
    let text = ui
        .message
        .msg_profile
        .replace("{telegram_id}", &snapshot.telegram_id.to_string())
        .replace("{balance}", &format_rub_exact(snapshot.balance))
        .replace("{subscription}", &subscription_line);
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_tariffs.clone(),
            "menu:tariffs",
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_menu_main.clone(),
            "menu:main",
        )],
    ]);
    MessageView { text, keyboard }
}
