use crate::domain::error::{DomainError, DomainResult};
use crate::domain::subscription::{Subscription, SubscriptionRepository};
use crate::domain::user::UserId;
use crate::exec_query;
use crate::infrastructure::database::{SharedTransaction, SqlxExecutor, SubscriptionRow};
use async_trait::async_trait;
use sqlx::PgPool;
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
        let query = sqlx::query!(
            r#"
            INSERT INTO subscriptions (user_id, plan, starts_at, expires_at, status, devices, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            sub.user_id().inner(),
            sub.plan().as_str(),
            sub.starts_at(),
            sub.expires_at(),
            sub.status().as_str(),
            sub.devices().inner(),
            sub.created_at()
        );

        exec_query!(self.executor, query, execute)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
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
            user_id.inner()
        );

        let row: Option<SubscriptionRow> =
            exec_query!(self.executor, query, fetch_optional)
                .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        let sub: Option<Subscription> = row.map(TryInto::try_into).transpose()?;

        Ok(sub)
    }
}
