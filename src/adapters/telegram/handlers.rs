pub mod callback;
pub mod command;
pub mod menu;
pub mod message;
pub mod state;

pub use callback::*;
pub use message::*;

pub struct MessageView {
    pub text: String,
    pub keyboard: teloxide::types::InlineKeyboardMarkup,
}
