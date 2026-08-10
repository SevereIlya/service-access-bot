use super::command::handle_commands;
use super::state::{try_handle_broadcast, try_handle_user_state};
use crate::adapters::telegram::BotState;
use crate::adapters::telegram::commands::Command;
use crate::adapters::telegram::error::TelegramResult;
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::{info, instrument};

#[instrument(skip_all, fields(telegram_id = msg.chat.id.0))]
pub async fn message_handler(
    bot: Bot,
    msg: Message,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat.id;
    let telegram_id = chat_id.0;

    let user_opt = state.usecases.get_user.execute(telegram_id).await?;

    // Пр1 - Рассылка
    if try_handle_broadcast(&bot, &msg, &state, &user_opt).await? {
        return Ok(());
    }

    // Пр2 - Тикеты
    if try_handle_user_state(&bot, &msg, &state, &user_opt).await? {
        return Ok(());
    }

    // Пр3 - Команды
    if let Some(text) = msg.text()
        && let Ok(cmd) = Command::parse(text, &state.bot_username)
    {
        info!(command = ?cmd, "Пользователь вызвал команду");
        handle_commands(bot, msg, state, user_opt, cmd).await?;
        return Ok(());
    }

    bot.send_message(
        chat_id,
        state.ui.message.msg_unknown_command.clone(),
    )
    .await?;

    Ok(())
}
