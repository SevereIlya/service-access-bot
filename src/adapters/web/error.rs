use crate::application::AppError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::{error, warn};

pub type WebResult<T> = Result<T, WebError>;

#[derive(Error, Debug)]
pub enum WebError {
    #[error("Not found")]
    NotFound,
    #[error("Forbidden")]
    Forbidden,

    #[error(transparent)]
    App(#[from] AppError),
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::App(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        match &self {
            Self::App(e) => error!(error = %e, "Веб-адаптер: сбой обработки запроса"),
            Self::Forbidden | Self::NotFound => warn!(error = %self, "Веб-адаптер отклонил запрос"),
        }
        status.into_response()
    }
}
