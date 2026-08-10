use crate::domain::error::DomainError;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("Max retries exceeded: {0}")]
    MaxRetriesExceeded(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
