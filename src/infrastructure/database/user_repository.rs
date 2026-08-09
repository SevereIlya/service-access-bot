use crate::domain::error::{DomainError, DomainResult};
use crate::domain::user::{TelegramId, User, UserId, UserRepository};
use crate::infrastructure::database::{SharedTransaction, SqlxExecutor, UserRow};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

pub struct SqlxUserRepository {
    executor: SqlxExecutor,
}

impl SqlxUserRepository {
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
impl UserRepository for SqlxUserRepository {
    #[instrument(skip(self, user), fields(telegram_id = %user.telegram_id()))]
    async fn create(&self, user: &User) -> DomainResult<UserId> {
        let query = sqlx::query!(
            r#"
            INSERT INTO users (
                uuid, telegram_id, username, full_name, role,
                frozen_base_price, referral_code, subscription_token,
                trial_used, discount_percent, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
            user.uuid(),
            user.telegram_id().0,
            user.username(),
            user.full_name(),
            user.role().as_str(),
            user.frozen_base_price().0,
            user.referral_code().0,
            user.subscription_token().0,
            user.trial_used(),
            i32::from(user.discount_percent().0),
            user.created_at(),
        );

        let result = match &self.executor {
            SqlxExecutor::Pool(pool) => query.fetch_one(pool).await,
            SqlxExecutor::Transaction(tx_mutex) => {
                let mut lock = tx_mutex.lock().await;
                if let Some(tx) = lock.as_mut() {
                    query.fetch_one(&mut **tx).await
                } else {
                    return Err(DomainError::SystemFailure("Транзакция закрыта".into()));
                }
            }
        };

        match result {
            Ok(record) => Ok(UserId(record.id)),
            Err(e) => {
                if let sqlx::Error::Database(db_err) = &e
                    && db_err.code().as_deref() == Some("23505")
                    && db_err.constraint()
                        == Some("users_referral_code_unique")
                {
                    return Err(DomainError::ReferralCodeCollision);
                }
                Err(DomainError::SystemFailure(e.to_string()))
            }
        }
    }

    #[instrument(skip(self, user), fields(telegram_id = %user.telegram_id()))]
    async fn update(&self, user: &User) -> DomainResult<()> {
        let id = user.id().ok_or(DomainError::EntityNotSaved)?;

        let query = sqlx::query!(
            r#"
            UPDATE users
            SET role = $1,
                frozen_base_price = $2,
                trial_used = $3,
                discount_percent = $4,
                username = $5,
                full_name = $6
            WHERE id = $7
            "#,
            user.role().as_str(),
            user.frozen_base_price().0,
            user.trial_used(),
            i32::from(user.discount_percent().0),
            user.username(),
            user.full_name(),
            id.0,
        );

        let result = match &self.executor {
            SqlxExecutor::Pool(pool) => query.execute(pool).await,
            SqlxExecutor::Transaction(tx_mutex) => {
                let mut lock = tx_mutex.lock().await;
                if let Some(tx) = lock.as_mut() {
                    query.execute(&mut **tx).await
                } else {
                    return Err(DomainError::SystemFailure("Транзакция закрыта".into()));
                }
            }
        };
        result.map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn find_by_user_id(&self, id: UserId) -> DomainResult<Option<User>> {
        let query = sqlx::query_as!(
            UserRow,
            r#"
            SELECT *
            FROM users
            WHERE id = $1
            "#,
            id.0
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

        let row: Option<UserRow> =
            result.map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        let user: Option<User> = row.map(TryInto::try_into).transpose()?;

        Ok(user)
    }

    #[instrument(skip(self))]
    async fn find_by_telegram_id(
        &self,
        telegram_id: TelegramId,
    ) -> DomainResult<Option<User>> {
        let query = sqlx::query_as!(
            UserRow,
            r#"
            SELECT *
            FROM users
            WHERE telegram_id = $1
            "#,
            telegram_id.0
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

        let row: Option<UserRow> =
            result.map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        let user: Option<User> = row.map(TryInto::try_into).transpose()?;

        Ok(user)
    }
}
