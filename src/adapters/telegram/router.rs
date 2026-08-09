use super::BotState;
use super::handlers::{callback_handler, message_handler};
use teloxide::prelude::*;
use tracing::info;

pub async fn start_bot(bot: Bot, state: BotState) {
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    info!("Telegram-бот готов к приему сообщений");

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
