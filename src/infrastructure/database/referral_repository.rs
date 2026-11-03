use crate::domain::{
    DomainError, DomainResult, ReferralError,
    referral::{Referral, ReferralId, ReferralRepository},
    user::UserId,
};
use crate::exec_query;
use crate::infrastructure::{ReferralRow, SharedTransaction, SqlxExecutor};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

#[must_use]
pub struct SqlxReferralRepository {
    executor: SqlxExecutor,
}

impl SqlxReferralRepository {
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
impl ReferralRepository for SqlxReferralRepository {
    #[instrument(
        skip(self, referral),
        fields(referrer_id = %referral.referrer_id(), referred_id = %referral.referred_id())
    )]
    async fn create(&self, referral: &Referral) -> DomainResult<ReferralId> {
        let query = sqlx::query!(
            r#"
            INSERT INTO referrals (referrer_id, referred_id, created_at)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            referral.referrer_id().inner(),
            referral.referred_id().inner(),
            referral.created_at(),
        );

        match exec_query!(self.executor, query, fetch_one) {
            Ok(record) => Ok(ReferralId::new(record.id)),
            Err(e) => {
                if let sqlx::Error::Database(db_err) = &e
                    && db_err.code().as_deref() == Some("23505")
                    && db_err.constraint() == Some("referrals_referred_id_unique")
                {
                    return Err(DomainError::Referral(ReferralError::AlreadyReferred));
                }
                Err(DomainError::SystemFailure(e.to_string()))
            }
        }
    }

    #[instrument(skip(self))]
    async fn find_by_referred_id(&self, referred_id: UserId) -> DomainResult<Option<Referral>> {
        let query = sqlx::query_as!(
            ReferralRow,
            r#"
            SELECT *
            FROM referrals
            WHERE referred_id = $1
            "#,
            referred_id.inner()
        );

        let row: Option<ReferralRow> = exec_query!(self.executor, query, fetch_optional)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        row.map(TryInto::try_into).transpose()
    }

    #[instrument(skip(self))]
    async fn count_by_referrer(&self, referrer_id: UserId) -> DomainResult<i64> {
        let query = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM referrals
            WHERE referrer_id = $1
            "#,
            referrer_id.inner()
        );

        exec_query!(self.executor, query, fetch_one)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))
    }
}
