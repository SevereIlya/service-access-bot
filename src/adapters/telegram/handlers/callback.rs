use crate::adapters::telegram::callbacks::CallbackAction;
use crate::adapters::telegram::error::TelegramResult;
use crate::adapters::telegram::handlers::menu::handle_menu;
use crate::adapters::telegram::BotState;
use teloxide::prelude::*;
use tracing::{info, warn, instrument};

#[instrument(skip_all, fields(telegram_id = qry.from.id.0))]
pub async fn callback_handler(
    bot: Bot,
    qry: CallbackQuery,
    state: BotState,
) -> TelegramResult<()> {
    let Some(data) = qry.data.clone() else {
        return Ok(())
    };

    let Some(msg) = qry.message.clone() else {
        return Ok(())
    };

    bot.answer_callback_query(qry.id.clone()).await?;
    let action = CallbackAction::parse(&data);
    info!(action = ?action, "Получен callback от пользователя");

    match action {
        CallbackAction::Menu(menu_action) => {
            handle_menu(bot.clone(), &qry, msg, state, menu_action).await?;
        }
        CallbackAction::Ignore => {}
        CallbackAction::Unknown(unparsed) => {
            warn!(
                callback = unparsed,
                "Прилетела неизвестная или битая кнопка"
            );
        }
    }
    Ok(())
}
