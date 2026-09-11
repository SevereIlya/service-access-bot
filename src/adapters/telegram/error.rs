use crate::application::AppError;
use thiserror::Error;

pub type TelegramResult<T> = Result<T, TelegramError>;

#[derive(Error, Debug)]
pub enum TelegramError {
    #[error("Telegram API error: {0}")]
    Api(#[from] teloxide::RequestError),

    #[error(transparent)]
    App(#[from] AppError),
}
