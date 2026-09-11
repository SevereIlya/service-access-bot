use crate::adapters::telegram::{MessageView, UiText, plan_label};
use crate::domain::subscription::SubscriptionPlan;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

#[must_use]
pub fn build_tariffs_view(ui: &UiText) -> MessageView {
    let plans = [
        (
            SubscriptionPlan::Week1,
            ui.button.btn_tariff_week_1.clone()
        ),
        (
            SubscriptionPlan::Month1,
            ui.button.btn_tariff_month_1.clone(),
        ),
        (
            SubscriptionPlan::Month3,
            ui.button.btn_tariff_month_3.clone(),
        ),
        (
            SubscriptionPlan::Month6,
            ui.button.btn_tariff_month_6.clone(),
        ),
    ];
    let mut rows: Vec<Vec<InlineKeyboardButton>> = plans
        .into_iter()
        .map(|(plan, label)| {
            vec![InlineKeyboardButton::callback(
                label,
                format!("purchase:view:{}", plan.as_str()),
            )]
        })
        .collect();
    rows.push(vec![InlineKeyboardButton::callback(
        ui.button.btn_menu_main.clone(),
        "menu:main",
    )]);

    MessageView {
        text: ui.message.msg_tariffs_select.clone(),
        keyboard: InlineKeyboardMarkup::new(rows),
    }
}

#[must_use]
pub fn build_tariff_details_view(ui: &UiText, plan: SubscriptionPlan) -> MessageView {
    let plan_name = plan_label(ui, plan);
    let price = placeholder_price_rub(plan);
    let text = ui
        .message
        .msg_tariff_details
        .replace("{plan}", &plan_name)
        .replace("{price}", &price.to_string());
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            ui.button.btn_pay.clone(),
            format!("purchase:pay:{}", plan.as_str()),
        )],
        vec![InlineKeyboardButton::callback(
            ui.button.btn_back.clone(),
            "menu:tariffs",
        )],
    ]);
    MessageView { text, keyboard }
}

const fn placeholder_price_rub(plan: SubscriptionPlan) -> u32 {
    match plan {
        SubscriptionPlan::Trial => 0,
        SubscriptionPlan::Week1 => 70,
        SubscriptionPlan::Month1 => 250,
        SubscriptionPlan::Month3 => 700,
        SubscriptionPlan::Month6 => 1200,
    }
}
