use crate::adapters::StubText;
use crate::application::UseCases;
use crate::infrastructure::AppConfig;
use std::sync::Arc;

pub mod error;
pub mod handlers;
pub mod router;

pub use error::*;
pub use handlers::*;
pub use router::*;

#[derive(Clone)]
pub struct WebState {
    pub usecases: Arc<UseCases>,
    pub config: Arc<AppConfig>,
    pub stub_text: Arc<StubText>,
}
