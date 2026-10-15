use crate::application::AppResult;
use crate::domain::billing::{DynTransactionRepository, Money, calculate_price};
use crate::domain::subscription::{SubscriptionDevices, SubscriptionPlan};
use crate::domain::user::User;
use crate::domain::{DomainError, UserError};

pub struct PurchaseQuote {
    pub price: Money,
    pub balance: Money,
    pub shortfall: Option<Money>,
}

pub struct GetPurchaseQuoteQuery {
    transaction_repo: DynTransactionRepository,
}

impl GetPurchaseQuoteQuery {
    pub fn new(transaction_repo: DynTransactionRepository) -> Self {
        Self { transaction_repo }
    }

    pub async fn execute(
        &self,
        user: &User,
        plan: SubscriptionPlan,
        devices: i32,
    ) -> AppResult<PurchaseQuote> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;
        let devices = SubscriptionDevices::new(devices)?;
        let price = calculate_price(plan, devices, user)?;
        let balance = self.transaction_repo.balance_for_user(user_id).await?;

        let shortfall = if balance < price {
            Some(Money::new(price.inner() - balance.inner())?.ceil_to_rubles())
        } else {
            None
        };

        Ok(PurchaseQuote {
            price,
            balance,
            shortfall,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        billing::{Transaction, TransactionId, TransactionRepository},
        user::{ReferralCode, SubscriptionToken, TelegramId, UserId},
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MockTransactionRepository {
        balance: Money,
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
            Ok(self.balance)
        }
    }

    fn create_test_user() -> User {
        let mut user = User::new(
            TelegramId::new(555),
            Uuid::new_v4(),
            Some("test".into()),
            "Test User".into(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        user.assign_id(UserId::new(1));
        user
    }

    #[tokio::test]
    async fn test_quote_has_no_shortfall_when_balance_enough() {
        let user = create_test_user();
        let plan = SubscriptionPlan::Month1;
        let expected_price =
            calculate_price(plan, SubscriptionDevices::new(1).unwrap(), &user).unwrap();

        let query = GetPurchaseQuoteQuery::new(Arc::new(MockTransactionRepository {
            balance: expected_price,
        }));

        let quote = query.execute(&user, plan, 1).await.unwrap();

        assert_eq!(quote.price, expected_price);
        assert!(
            quote.shortfall.is_none(),
            "баланс равен цене - нехватки быть не должно"
        );
    }

    #[tokio::test]
    async fn test_quote_rounds_shortfall_up_to_whole_ruble() {
        let user = create_test_user();
        let plan = SubscriptionPlan::Month1;
        let price = calculate_price(plan, SubscriptionDevices::new(1).unwrap(), &user).unwrap();
        let balance = Money::new(price.inner() - 5_037).unwrap();

        let query = GetPurchaseQuoteQuery::new(Arc::new(MockTransactionRepository { balance }));
        let quote = query.execute(&user, plan, 1).await.unwrap();

        let shortfall = quote.shortfall.expect("баланс меньше цены - нехватка должна считаться");
        assert_eq!(
            shortfall.inner() % 100,
            0,
            "нехватка должна быть округлена до целого рубля - иначе разъедется с реальным инвойсом Platega"
        );
        assert!(
            shortfall.inner() >= 5_037,
            "округление ВВЕРХ не должно давать меньше точной нехватки"
        );
        assert!(
            shortfall.inner() < 5_037 + 100,
            "округление не должно завышать больше чем на 99 копеек"
        );
    }

    #[tokio::test]
    async fn test_quote_fails_for_invalid_devices() {
        let user = create_test_user();
        let query = GetPurchaseQuoteQuery::new(Arc::new(MockTransactionRepository {
            balance: Money::new(0).unwrap(),
        }));

        let result = query.execute(&user, SubscriptionPlan::Month1, 0).await;
        assert!(result.is_err(), "0 устройств - невалидное значение");
    }
}
