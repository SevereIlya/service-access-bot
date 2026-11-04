use crate::adapters::StubText;
use crate::application::UseCases;
use std::net::IpAddr;
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
    pub webhook_allowed_ips: Arc<[IpAddr]>,
    pub stub_text: Arc<StubText>,
}
