use teloxide::Bot;
use teloxide::prelude::Message;
use crate::adapters::telegram::BotState;
use crate::adapters::telegram::error::TelegramResult;
use crate::domain::user::entity::User;

pub async fn try_handle_broadcast(
    _bot: &Bot,
    _msg: &Message,
    _state: &BotState,
    _user_opt: &Option<User>,
) -> TelegramResult<bool> {
    // TODO: Сделать
    Ok(false)
}

pub async fn try_handle_user_state(
    _bot: &Bot,
    _msg: &Message,
    _state: &BotState,
    _user_opt: &Option<User>,
) -> TelegramResult<bool> {
    // TODO: Сделать
    Ok(false)
}

