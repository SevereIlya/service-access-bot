use crate::adapters::telegram::{MessageView, UiText, format_rub, plan_label};
use crate::application::{AppResult, PurchaseQuote};
use crate::domain::{
    billing::{Money, calculate_price},
    subscription::{SubscriptionDevices, SubscriptionPlan},
    user::User,
};
use chrono::{DateTime, FixedOffset, Utc};
use reqwest::Url;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const DEVICE_TIERS: [i32; 4] = [1, 3, 5, 10];

const PLANS: [SubscriptionPlan; 4] = [
    SubscriptionPlan::Week1,
    SubscriptionPlan::Month1,
    SubscriptionPlan::Month3,
    SubscriptionPlan::Month6,
];

fn device_tier_step(current: i32, delta: i32) -> i32 {
    let idx = DEVICE_TIERS.iter().position(|&tier| tier == current).unwrap_or(0);
    let delta = isize::try_from(delta).expect("i32 always fits into isize");
    let new_idx = idx.saturating_add_signed(delta).min(DEVICE_TIERS.len() - 1);
    DEVICE_TIERS[new_idx]
}

/// Матрица тарифов
pub fn build_tariffs_view(ui: &UiText, user: &User, devices: i32) -> AppResult<MessageView> {
    let device_count = SubscriptionDevices::new(devices)?;

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::with_capacity(6);

    rows.push(vec![
        InlineKeyboardButton::callback(
            "➖",
            format!("menu:tariffs:{}", device_tier_step(devices, -1)),
        ),
        InlineKeyboardButton::callback(format!("📱 {devices}"), "ignore"),
        InlineKeyboardButton::callback(
            "➕",
            format!("menu:tariffs:{}", device_tier_step(devices, 1)),
        ),
    ]);

    for plan in PLANS {
        let price = calculate_price(plan, device_count, user)?;
        let label = format!("{} — {}₽", plan_label(ui, plan), format_rub(price));
        rows.push(vec![InlineKeyboardButton::callback(
            label,
            format!("purchase:view:{}:{devices}", plan.as_str()),
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]);

    Ok(MessageView {
        text: ui.message.msg_tariffs_select.clone(),
        keyboard: InlineKeyboardMarkup::new(rows),
    })
}

/// Экран подтверждения тарифа.
/// Xватает баланса - Оплатить, не хватает - Пополнить {X}₽.
#[must_use]
pub fn build_purchase_quote_view(
    ui: &UiText,
    plan: SubscriptionPlan,
    devices: i32,
    quote: &PurchaseQuote,
) -> MessageView {
    let plan_name = plan_label(ui, plan);
    let price_str = format_rub(quote.price);

    let (text, action_button) = quote.shortfall.map_or_else(
        || {
            let text = ui
                .message
                .msg_purchase_confirm
                .replace("{plan}", &plan_name)
                .replace("{devices}", &devices.to_string())
                .replace("{price}", &price_str);
            let btn = InlineKeyboardButton::callback(
                ui.button.btn_pay.clone(),
                format!("purchase:pay:{}:{devices}", plan.as_str()),
            );
            (text, btn)
        },
        |shortfall| {
            let shortfall_str = format_rub(shortfall);
            let text = ui
                .message
                .msg_purchase_insufficient
                .replace("{plan}", &plan_name)
                .replace("{devices}", &devices.to_string())
                .replace("{price}", &price_str)
                .replace("{shortfall}", &shortfall_str);
            let btn = InlineKeyboardButton::callback(
                ui.button.btn_top_up_amount.replace("{amount}", &shortfall_str),
                format!("purchase:topup:{}:{devices}", plan.as_str()),
            );
            (text, btn)
        },
    );

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![action_button],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_back.clone(),
            format!("menu:tariffs:{devices}"),
        )],
    ]);

    MessageView { text, keyboard }
}

/// Экран со ссылкой на оплату
#[must_use]
pub fn build_top_up_link_view(
    ui: &UiText,
    payment_url: Url,
    shortfall: Money,
    plan: SubscriptionPlan,
    devices: i32,
) -> MessageView {
    let text = ui.message.msg_top_up_link.replace("{amount}", &format_rub(shortfall));
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::url(
            ui.button.btn_open_payment.clone(),
            payment_url,
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_back.clone(),
            format!("purchase:view:{}:{devices}", plan.as_str()),
        )],
    ]);
    MessageView { text, keyboard }
}

/// Экран успеха
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn build_purchase_success_view(
    ui: &UiText,
    plan: SubscriptionPlan,
    devices: SubscriptionDevices,
    expires_at: DateTime<Utc>,
) -> MessageView {
    let msk_offset = FixedOffset::east_opt(3 * 3600)
        .expect("Hardcoded offset of +3 hours is mathematically always valid");
    let date_str = expires_at.with_timezone(&msk_offset).format("%d.%m.%Y %H:%M").to_string();

    let text = ui
        .message
        .msg_purchase_success
        .replace("{plan}", &plan_label(ui, plan))
        .replace("{devices}", &devices.to_string())
        .replace("{date}", &date_str);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_tier_step_clamps_at_bounds() {
        assert_eq!(
            device_tier_step(1, -1),
            1,
            "нельзя уйти ниже минимального тира"
        );
        assert_eq!(
            device_tier_step(10, 1),
            10,
            "нельзя уйти выше максимального тира"
        );
    }

    #[test]
    fn test_device_tier_step_moves_between_tiers() {
        assert_eq!(device_tier_step(1, 1), 3);
        assert_eq!(device_tier_step(3, 1), 5);
        assert_eq!(device_tier_step(5, 1), 10);
        assert_eq!(device_tier_step(10, -1), 5);
    }
}
