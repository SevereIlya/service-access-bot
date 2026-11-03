use crate::domain::{
    DomainError, DomainResult,
    billing::{Money, Transaction, TransactionId, TransactionKind, TransactionRepository},
    user::UserId,
};
use crate::exec_query;
use crate::infrastructure::{SharedTransaction, SqlxExecutor, TransactionRow};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

#[must_use]
pub struct SqlxTransactionRepository {
    executor: SqlxExecutor,
}

impl SqlxTransactionRepository {
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
            <&'static str>::from(tx.kind()),
            <&'static str>::from(tx.status()),
            <&'static str>::from(tx.provider()),
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
            <&'static str>::from(tx.status()),
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
    async fn find_by_provider_tx_id(
        &self,
        provider_tx_id: &str,
    ) -> DomainResult<Option<Transaction>> {
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

        let credit_kinds: Vec<String> =
            TransactionKind::credit_kind_strings().into_iter().map(String::from).collect();

        // Читаем баланс уже после блокировки, чтобы отловить все изменения
        let balance = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(
                CASE WHEN kind = ANY($1::TEXT[])
                     THEN amount ELSE -amount END
            ), 0)::BIGINT AS "balance!: i64"
            FROM transactions
            WHERE user_id = $2 AND status = 'success'
            "#,
            &credit_kinds[..],
            user_id.inner(),
        );

        let value: i64 = exec_query!(self.executor, balance, fetch_one)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        Money::new(value).map_err(|_| {
            DomainError::SystemFailure(format!("Баланс юзера {user_id} ушёл в минус: {value}"))
        })
    }

    #[instrument(skip(self))]
    async fn sum_for_user_by_kind(
        &self,
        user_id: UserId,
        kind: TransactionKind,
    ) -> DomainResult<Money> {
        let query = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(amount), 0)::BIGINT AS "total!: i64"
            FROM transactions
            WHERE user_id = $1 AND kind = $2 AND status = 'success'
            "#,
            user_id.inner(),
            <&'static str>::from(kind),
        );

        let value: i64 = exec_query!(self.executor, query, fetch_one)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;

        Money::new(value).map_err(|_| {
            DomainError::SystemFailure(format!(
                "Сумма транзакций юзера {user_id} ушла в минус: {value}"
            ))
        })
    }
}
