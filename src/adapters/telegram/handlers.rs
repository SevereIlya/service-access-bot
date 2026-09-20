mod callback;
mod command;
mod menu;
mod message;
mod profile;
mod purchase;

use crate::adapters::telegram::TelegramResult;
pub use callback::callback_handler;
pub use message::message_handler;
use teloxide::{
    ApiError, RequestError,
    prelude::*,
    sugar::request::RequestLinkPreviewExt,
    types::{MaybeInaccessibleMessage, ParseMode::Html},
};

pub struct MessageView {
    pub text: String,
    pub keyboard: teloxide::types::InlineKeyboardMarkup,
}

pub(crate) async fn render_view(
    bot: &Bot,
    msg: &MaybeInaccessibleMessage,
    view: MessageView,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let is_media = msg
        .regular_message()
        .is_some_and(|m| m.photo().is_some() || m.document().is_some());
    if is_media {
        let _ = bot.delete_message(chat_id, message_id).await;
        bot.send_message(chat_id, view.text)
            .parse_mode(Html)
            .disable_link_preview(true)
            .reply_markup(view.keyboard)
            .await?;
    } else {
        let result = bot
            .edit_message_text(chat_id, message_id, view.text)
            .parse_mode(Html)
            .disable_link_preview(true)
            .reply_markup(view.keyboard)
            .await;

        if let Err(RequestError::Api(ApiError::MessageNotModified)) = result {
            return Ok(());
        }
        result?;
    }
    Ok(())
}
