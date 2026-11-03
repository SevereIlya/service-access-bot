use crate::application::AppResult;
use crate::domain::{
    DomainError, UserError,
    billing::{DynTransactionRepository, Money, TransactionKind},
    referral::{DynReferralRepository, ReferralPolicy},
    user::User,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferralOverview {
    pub welcome_bonus: Money,
    pub cashback_percent: i32,
    pub invited: i64,
    pub earned: Money,
}

pub struct GetReferralOverviewQuery {
    referral_repo: DynReferralRepository,
    transaction_repo: DynTransactionRepository,
    policy: ReferralPolicy,
}

impl GetReferralOverviewQuery {
    pub fn new(
        referral_repo: DynReferralRepository,
        transaction_repo: DynTransactionRepository,
        policy: ReferralPolicy,
    ) -> Self {
        Self {
            referral_repo,
            transaction_repo,
            policy,
        }
    }

    pub async fn execute(&self, user: &User) -> AppResult<ReferralOverview> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;

        let (invited, earned) = tokio::try_join!(
            self.referral_repo.count_by_referrer(user_id),
            self.transaction_repo
                .sum_for_user_by_kind(user_id, TransactionKind::ReferralCashback),
        )?;

        Ok(ReferralOverview {
            welcome_bonus: self.policy.welcome_bonus(),
            cashback_percent: i32::from(self.policy.cashback_percent().inner()),
            invited,
            earned,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::AppError;
    use crate::domain::{
        DomainResult,
        billing::{Transaction, TransactionId, TransactionRepository},
        referral::{CashbackPercent, Referral, ReferralId, ReferralRepository},
        user::{ReferralCode, SubscriptionToken, TelegramId, UserId},
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MockReferralRepository {
        invited: i64,
    }

    #[async_trait]
    impl ReferralRepository for MockReferralRepository {
        async fn create(&self, _referral: &Referral) -> DomainResult<ReferralId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn find_by_referred_id(
            &self,
            _referred_id: UserId,
        ) -> DomainResult<Option<Referral>> {
            unreachable!("find_by_referred_id() не используется в этом юзкейсе")
        }
        async fn count_by_referrer(&self, referrer_id: UserId) -> DomainResult<i64> {
            assert_eq!(
                referrer_id,
                UserId::new(1),
                "считать надо приглашённых самого пользователя"
            );
            Ok(self.invited)
        }
    }

    struct MockTransactionRepository {
        earned: Money,
    }

    #[async_trait]
    impl TransactionRepository for MockTransactionRepository {
        async fn create(&self, _tx: &Transaction) -> DomainResult<TransactionId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _tx: &Transaction) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }
        async fn find_by_order_id(&self, _order_id: &str) -> DomainResult<Option<Transaction>> {
            unreachable!("find_by_order_id() не используется в этом юзкейсе")
        }
        async fn find_by_provider_tx_id(
            &self,
            _provider_tx_id: &str,
        ) -> DomainResult<Option<Transaction>> {
            unreachable!("find_by_provider_tx_id() не используется в этом юзкейсе")
        }
        async fn balance_for_user(&self, _user_id: UserId) -> DomainResult<Money> {
            unreachable!("balance_for_user() не используется в этом юзкейсе")
        }
        async fn sum_for_user_by_kind(
            &self,
            user_id: UserId,
            kind: TransactionKind,
        ) -> DomainResult<Money> {
            assert_eq!(user_id, UserId::new(1));
            assert_eq!(
                kind,
                TransactionKind::ReferralCashback,
                "в заработок идёт только кэшбек: не приветственный бонус и не пополнения"
            );
            Ok(self.earned)
        }
    }

    fn create_test_user(has_id: bool) -> User {
        let mut user = User::new(
            TelegramId::new(555),
            Uuid::new_v4(),
            Some("test".into()),
            "Test User".into(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        if has_id {
            user.assign_id(UserId::new(1));
        }
        user
    }

    fn build_query() -> GetReferralOverviewQuery {
        GetReferralOverviewQuery::new(
            Arc::new(MockReferralRepository { invited: 3 }),
            Arc::new(MockTransactionRepository {
                earned: Money::new(1_740).unwrap(),
            }),
            ReferralPolicy::new(
                Money::new(3_800).unwrap(),
                CashbackPercent::new(10).unwrap(),
            ),
        )
    }

    #[tokio::test]
    async fn test_overview_combines_stats_and_policy_terms() {
        let overview = build_query().execute(&create_test_user(true)).await.unwrap();

        assert_eq!(
            overview,
            ReferralOverview {
                welcome_bonus: Money::new(3_800).unwrap(),
                cashback_percent: 10,
                invited: 3,
                earned: Money::new(1_740).unwrap(),
            }
        );
    }

    #[tokio::test]
    async fn test_overview_fails_when_user_not_saved() {
        let result = build_query().execute(&create_test_user(false)).await;

        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::User(UserError::EntityNotSaved))
        ));
    }
}
