use crate::domain::error::{DomainError, DomainResult};
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::uow::{BoxedUowContext, UnitOfWork, UowContext};
use crate::domain::user::DynUserRepository;
use crate::infrastructure::database::{
    SharedTransaction, SqlxSubscriptionRepository, SqlxUserRepository,
};
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

pub struct SqlxUnitOfWork {
    pool: PgPool,
}

impl SqlxUnitOfWork {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UnitOfWork for SqlxUnitOfWork {
    async fn begin(&self) -> DomainResult<BoxedUowContext> {
        let tx: Transaction<Postgres> = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        let shared_tx = Arc::new(Mutex::new(Some(tx)));
        Ok(Box::new(SqlxUowContext { tx: shared_tx }))
    }
}

pub struct SqlxUowContext {
    tx: SharedTransaction,
}

#[async_trait]
impl UowContext for SqlxUowContext {
    fn users(&self) -> DynUserRepository {
        Arc::new(SqlxUserRepository::transaction(self.tx.clone()))
    }

    fn subscriptions(&self) -> DynSubscriptionRepository {
        Arc::new(SqlxSubscriptionRepository::transaction(self.tx.clone()))
    }

    async fn commit(&mut self) -> DomainResult<()> {
        // Пишу подробно чтобы иметь в виду в будущем
        // Ограничиваю жизнь лока блоком {}
        let tx_opt = {
            let mut tx_lock: MutexGuard<Option<Transaction<'static, Postgres>>> =
                self.tx.lock().await;
            // Забираем Transaction, оставляем None
            tx_lock.take()
        };
        // Все, tx_lock умер. Мьютекс разблокирован

        // У нас есть транзакция без лока
        if let Some(tx) = tx_opt {
            let tx: Transaction<'static, Postgres> = tx;
            tx.commit().await.map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        }
        Ok(())
    }

    async fn rollback(&mut self) -> DomainResult<()> {
        let tx_opt = {
            let mut tx_lock = self.tx.lock().await;
            tx_lock.take()
        };

        if let Some(tx) = tx_opt {
            tx.rollback().await.map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        }
        Ok(())
    }
}
