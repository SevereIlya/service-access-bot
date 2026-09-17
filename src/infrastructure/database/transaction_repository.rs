use crate::domain::{
    DomainError, DomainResult,
    billing::{Money, Transaction, TransactionId, TransactionRepository},
    user::UserId,
};
use crate::exec_query;
use crate::infrastructure::{SharedTransaction, SqlxExecutor, TransactionRow};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

pub struct SqlxTransactionRepository {
    executor: SqlxExecutor,
}

impl SqlxTransactionRepository {
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
impl TransactionRepository for SqlxTransactionRepository {
    #[instrument(skip(self, tx), fields(order_id = %tx.order_id()))]
    async fn create(&self, tx: &Transaction) -> DomainResult<TransactionId> {
        let query = sqlx::query!(
            r#"
            INSERT INTO transactions (user_id, amount, kind, status, payment_provider, order_id, provider_tx_id, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
            tx.user_id().inner(),
            tx.amount().inner(),
            tx.kind().as_str(),
            tx.status().as_str(),
            tx.provider().as_str(),
            tx.order_id(),
            tx.provider_tx_id(),
            tx.payload().cloned(),
            tx.created_at(),
        );

        let record = exec_query!(self.executor, query, fetch_one)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        Ok(TransactionId::new(record.id))
    }

    #[instrument(skip(self, tx), fields(order_id = %tx.order_id()))]
    async fn update(&self, tx: &Transaction) -> DomainResult<()> {
        let query = sqlx::query!(
            r#"
            UPDATE transactions
            SET status = $1,
                provider_tx_id = $2,
                payload = $3
            WHERE order_id = $4
            "#,
            tx.status().as_str(),
            tx.provider_tx_id(),
            tx.payload().cloned(),
            tx.order_id(),
        );
        exec_query!(self.executor, query, execute)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn find_by_order_id(&self, order_id: &str) -> DomainResult<Option<Transaction>> {
        let query = sqlx::query_as!(
            TransactionRow,
            r#"
            SELECT *
            FROM transactions
            WHERE order_id = $1
            "#,
            order_id
        );
        let row: Option<TransactionRow> = exec_query!(self.executor, query, fetch_optional)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        row.map(TryInto::try_into).transpose()
    }

    #[instrument(skip(self))]
    async fn find_by_provider_tx_id(&self, provider_tx_id: &str) -> DomainResult<Option<Transaction>> {
        let query = sqlx::query_as!(
            TransactionRow,
            r#"
            SELECT *
            FROM transactions
            WHERE provider_tx_id = $1
            "#,
            provider_tx_id
        );
        let row: Option<TransactionRow> = exec_query!(self.executor, query, fetch_optional)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        row.map(TryInto::try_into).transpose()
    }

    #[instrument(skip(self))]
    async fn balance_for_user(&self, user_id: UserId) -> DomainResult<Money> {
        // Блокируем строку юзера до конца транзакции, чтобы избежать параллельного списания
        let lock = sqlx::query!(
            r#"
            SELECT id
            FROM users
            WHERE id = $1
            FOR UPDATE
            "#,
            user_id.inner()
        );
        let _ = exec_query!(self.executor, lock, fetch_one).map_err(|e| {
            DomainError::SystemFailure(format!("Не удалось заблокировать юзера {user_id}: {e}"))
        })?;

        // Читаем баланс уже после блокировки, чтобы отловить все изменения
        let balance = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(
                CASE WHEN kind IN ('top_up', 'referral_cashback', 'refund')
                     THEN amount ELSE -amount END
            ), 0)::BIGINT AS "balance!: i64"
            FROM transactions
            WHERE user_id = $1 AND status = 'success'
            "#,
            user_id.inner(),
        );

        let value: i64 = exec_query!(self.executor, balance, fetch_one)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        Money::new(value).map_err(|_| {
            DomainError::SystemFailure(format!("Баланс юзера {user_id} ушёл в минус: {value}"))
        })
    }
}
