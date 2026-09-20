use crate::adapters::telegram::{BotState, Command, TelegramResult, handlers::command};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::{error, info, instrument};

pub async fn message_handler(bot: Bot, msg: Message, state: BotState) -> TelegramResult<()> {
    let telegram_id = msg.chat.id.0;
    if let Err(e) = message_handler_inner(bot, msg, state).await {
        error!(error = %e, telegram_id, "Необработанная ошибка в message_handler");
    }
    Ok(())
}

#[instrument(skip_all, fields(telegram_id = msg.chat.id.0))]
pub async fn message_handler_inner(bot: Bot, msg: Message, state: BotState) -> TelegramResult<()> {
    let chat_id = msg.chat.id;
    let telegram_id = chat_id.0;

    let user_opt = state.usecases.get_user_by_telegram_id.execute(telegram_id).await?;

    if let Some(text) = msg.text()
        && let Ok(cmd) = Command::parse(text, &state.bot_username)
    {
        info!(command = ?cmd, "Пользователь вызвал команду");
        return command::handle_commands(bot, msg, state, user_opt, cmd).await;
    }

    bot.send_message(chat_id, state.ui_text.message.msg_unknown_command.clone()).await?;
    Ok(())
}
