pub mod mappers;
pub mod models;
pub mod postgres;
pub mod subscription_repository;
pub mod uow;
pub mod user_repository;

pub use models::*;
pub use postgres::*;
pub use subscription_repository::*;
pub use uow::*;
pub use user_repository::*;

use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type SharedTransaction = Arc<Mutex<Option<Transaction<'static, Postgres>>>>;

pub enum SqlxExecutor {
    Pool(PgPool),
    Transaction(SharedTransaction),
}