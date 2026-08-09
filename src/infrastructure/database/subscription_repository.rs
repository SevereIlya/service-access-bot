use crate::domain::error::{DomainError, DomainResult};
use crate::domain::subscription::{Subscription, SubscriptionRepository};
use crate::domain::user::UserId;
use crate::infrastructure::database::{SharedTransaction, SqlxExecutor, SubscriptionRow};
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;

pub struct SqlxSubscriptionRepository {
    executor: SqlxExecutor,
}

impl SqlxSubscriptionRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self {
            executor: SqlxExecutor::Pool(pool),
        }
    }

    pub const fn transaction(tx: SharedTransaction) -> Self {
        Self {
            executor: SqlxExecutor::Transaction(tx),
        }
    }
}

#[async_trait]
impl SubscriptionRepository for SqlxSubscriptionRepository {
    #[instrument(skip(self, sub), fields(user_id = %sub.user_id()))]
    async fn create(&self, sub: &Subscription) -> DomainResult<()> {
        let query1 = sqlx::query!(
            r#"
            UPDATE subscriptions
            SET status = 'inactive'
            WHERE user_id = $1 AND status = 'active'
            "#,
            sub.user_id().0
        );

        let query2 = sqlx::query!(
            r#"
            INSERT INTO subscriptions (user_id, plan, starts_at, expires_at, status, devices, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            sub.user_id().0,
            sub.plan().as_str(),
            sub.starts_at(),
            sub.expires_at(),
            sub.status().as_str(),
            sub.devices().0,
            sub.created_at()
        );

        match &self.executor {
            SqlxExecutor::Pool(pool) => {
                let mut tx: Transaction<Postgres> = pool
                    .begin()
                    .await
                    .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
                query1
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
                query2
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
            }
            SqlxExecutor::Transaction(tx_mutex) => {
                let mut lock = tx_mutex.lock().await;
                if let Some(tx) = lock.as_mut() {
                    query1
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
                    query2
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
                } else {
                    return Err(DomainError::SystemFailure("Транзакция закрыта".into()));
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn find_active_by_user_id(
        &self,
        user_id: UserId,
    ) -> DomainResult<Option<Subscription>> {
        let query = sqlx::query_as!(
            SubscriptionRow,
            r#"
            SELECT *
            FROM subscriptions
            WHERE user_id = $1
              AND status = 'active'
              AND NOW() < expires_at
            LIMIT 1
            "#,
            user_id.0
        );

        let result = match &self.executor {
            SqlxExecutor::Pool(pool) => query.fetch_optional(pool).await,
            SqlxExecutor::Transaction(tx_mutex) => {
                let mut lock = tx_mutex.lock().await;
                if let Some(tx) = lock.as_mut() {
                    query.fetch_optional(&mut **tx).await
                } else {
                    return Err(DomainError::SystemFailure("Транзакция закрыта".into()));
                }
            }
        };

        let row: Option<SubscriptionRow> =
            result.map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        let sub: Option<Subscription> = row.map(TryInto::try_into).transpose()?;

        Ok(sub)
    }
}
