use thiserror::Error;
use crate::application::error::AppError;

#[derive(Error, Debug)]
pub enum TelegramError {
    #[error("Telegram API error: {0}")]
    Api(#[from] teloxide::RequestError),

    #[error("Business logic error: {0}")]
    App(#[from] AppError),
}

pub type TelegramResult<T> = Result<T, TelegramError>;