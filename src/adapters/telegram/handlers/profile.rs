use crate::adapters::telegram::{BotState, TelegramResult, message_error, render_view, views};
use crate::application::AppError;
use crate::domain::{DomainError, UserError};
use teloxide::prelude::*;
use teloxide::types::MaybeInaccessibleMessage;
use tracing::warn;

pub async fn handle_profile(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let telegram_id = chat_id.0;
    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(UserError::NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(
            &state.ui_text,
            &AppError::Domain(DomainError::User(UserError::NotFound)),
        );
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };
    let snapshot = state.usecases.get_profile.execute(&user).await?;
    let view = views::build_profile_view(&state.ui_text, &snapshot);
    render_view(&bot, &msg, view).await
}
