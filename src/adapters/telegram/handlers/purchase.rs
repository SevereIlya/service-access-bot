use crate::adapters::telegram::{BotState, PurchaseAction, TelegramResult, views};
use crate::domain::subscription::SubscriptionPlan;
use teloxide::{prelude::*, types::MaybeInaccessibleMessage, types::ParseMode::Html};
use tracing::info;

pub async fn handle_tariffs(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let view = views::build_tariffs_view(&state.ui_text);

    let is_media = msg
        .regular_message()
        .is_some_and(|m| m.photo().is_some() || m.document().is_some());

    if is_media {
        let _ = bot.delete_message(chat_id, message_id).await;
        bot.send_message(chat_id, view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
    } else {
        bot.edit_message_text(chat_id, message_id, view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
    }
    Ok(())
}

pub async fn handle_purchase(
    bot: Bot,
    qry: &CallbackQuery,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    action: PurchaseAction,
) -> TelegramResult<()> {
    match action {
        PurchaseAction::View(plan) => {
            bot.answer_callback_query(qry.id.clone()).await?;
            show_tariff_details(bot, msg, state, plan).await?;
        }
        PurchaseAction::Pay(plan) => {
            info!(?plan, "Юзер нажал 'Оплатить' — шлюз ещё не подключен");
            bot.answer_callback_query(qry.id.clone())
                .show_alert(true)
                .text("⏳ Шлюз оплаты в процессе подключения. Попробуйте позже.")
                .await?;
        }
    }
    Ok(())
}

async fn show_tariff_details(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    plan: SubscriptionPlan,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let view = views::build_tariff_details_view(&state.ui_text, plan);

    bot.edit_message_text(chat_id, message_id, view.text)
        .parse_mode(Html)
        .reply_markup(view.keyboard)
        .await?;
    Ok(())
}
