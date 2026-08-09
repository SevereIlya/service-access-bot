use crate::infrastructure::database::{
    SqlxSubscriptionRepository, SqlxUnitOfWork, SqlxUserRepository,
};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::debug;

pub struct Repositories {
    pub user: Arc<SqlxUserRepository>,
    pub subscription: Arc<SqlxSubscriptionRepository>,
    pub uow: Arc<SqlxUnitOfWork>,
}
impl Repositories {
    pub fn new(pool: PgPool) -> Self {
        debug!("Инициализация репозиториев...");
        Self {
            user: Arc::new(SqlxUserRepository::new(pool.clone())),
            subscription: Arc::new(SqlxSubscriptionRepository::new(pool.clone())),
            uow: Arc::new(SqlxUnitOfWork::new(pool)),
        }
    }
}
