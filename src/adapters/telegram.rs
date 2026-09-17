pub mod callbacks;
pub mod commands;
pub mod error;
pub mod handlers;
pub mod notifier;
pub mod router;
pub mod ui;
pub mod views;
pub mod alerts;

pub use callbacks::*;
pub use commands::*;
pub use error::*;
pub use handlers::*;
pub use notifier::*;
pub use router::*;
pub use ui::*;
pub use views::*;
pub use alerts::*;

use crate::application::UseCases;
use std::sync::Arc;

#[derive(Clone)]
pub struct BotState {
    pub usecases: Arc<UseCases>,
    pub bot_username: String,
    pub ui_text: Arc<UiText>,
    pub admin_chat_id: i64,
    pub public_base_url: String,
}
