use crate::domain::error::{DomainError, DomainResult, SubscriptionError};
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
            INSERT INTO subscriptions (user_id, plan, starts_at, expires_at, status, devices, is_warning_sent, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            sub.user_id().inner(),
            sub.plan().as_str(),
            sub.starts_at(),
            sub.expires_at(),
            sub.status().as_str(),
            sub.devices().inner(),
            false,
            sub.created_at()
        );

        match exec_query!(self.executor, query, execute) {
            Ok(_) => Ok(()),
            Err(e) => {
                if let sqlx::Error::Database(db_err) = &e
                    && db_err.code().as_deref() == Some("23505")
                {
                    #[allow(clippy::single_match)]
                    match db_err.constraint() {
                        Some("idx_subscriptions_one_active") => {
                            return Err(DomainError::Subscription(
                                SubscriptionError::AlreadyHasActive,
                            ));
                        }
                        _ => {}
                    }
                }
                Err(DomainError::SystemFailure(e.to_string()))
            }
        }
    }

    #[instrument(skip(self, sub), fields(user_id = %sub.user_id()))]
    async fn update(&self, sub: &Subscription) -> DomainResult<()> {
        let id = sub
            .id()
            .ok_or(DomainError::Subscription(SubscriptionError::EntityNotSaved))?;

        let query = sqlx::query!(
            r#"
            UPDATE subscriptions
            SET plan = $1,
                expires_at = $2,
                status = $3,
                devices = $4,
                is_warning_sent = $5
            WHERE id = $6
            "#,
            sub.plan().as_str(),
            sub.expires_at(),
            sub.status().as_str(),
            sub.devices().inner(),
            sub.is_warning_sent(),
            id.inner(),
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

    #[instrument(skip(self))]
    async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
        let query = sqlx::query_as!(
            SubscriptionRow,
            r#"
            SELECT *
            FROM subscriptions
            WHERE status = 'active'
              AND expires_at <= NOW()
            "#
        );

        let rows: Vec<SubscriptionRow> = exec_query!(self.executor, query, fetch_all)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    #[instrument(skip(self))]
    async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
        let query = sqlx::query_as!(
            SubscriptionRow,
            r#"
            SELECT *
            FROM subscriptions
            WHERE status = 'active'
              AND is_warning_sent = false
              AND expires_at <= NOW() + INTERVAL '24 hours'
              AND expires_at > NOW()
            "#
        );

        let rows: Vec<SubscriptionRow> = exec_query!(self.executor, query, fetch_all)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}
