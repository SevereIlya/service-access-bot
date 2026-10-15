use crate::adapters::telegram::{
    BotState, CallbackAction, TelegramResult, handlers::menu, handlers::purchase,
};
use teloxide::prelude::*;
use tracing::{error, info, instrument, warn};

pub async fn callback_handler(bot: Bot, qry: CallbackQuery, state: BotState) -> TelegramResult<()> {
    let telegram_id = qry.from.id.0;
    if let Err(e) = callback_handler_inner(bot, qry, state).await {
        error!(error = %e, telegram_id, "Необработанная ошибка в callback_handler");
    }
    Ok(())
}

#[instrument(skip_all, fields(telegram_id = qry.from.id.0))]
pub async fn callback_handler_inner(
    bot: Bot,
    qry: CallbackQuery,
    state: BotState,
) -> TelegramResult<()> {
    let Some(data) = qry.data.clone() else {
        return Ok(());
    };

    let Some(msg) = qry.message.clone() else {
        return Ok(());
    };

    let action = CallbackAction::parse(&data);
    info!(action = ?action, "Получен callback от пользователя");

    match action {
        CallbackAction::Menu(menu_action) => {
            bot.answer_callback_query(qry.id.clone()).await?;
            menu::handle_menu(bot.clone(), &qry, msg, state, menu_action).await?;
        }
        CallbackAction::Purchase(purchase_action) => {
            purchase::handle_purchase(bot.clone(), &qry, msg, state, purchase_action).await?;
        }
        CallbackAction::Ignore => {
            bot.answer_callback_query(qry.id.clone()).await?;
        }
        CallbackAction::Unknown(unparsed) => {
            bot.answer_callback_query(qry.id.clone()).await?;
            warn!(
                callback = unparsed,
                "Прилетела неизвестная или битая кнопка"
            );
        }
    }
    Ok(())
}
