use crate::application::AppResult;
use crate::domain::{
    BillingError, DomainError, UserError,
    billing::{Money, Transaction, TransactionKind, calculate_price},
    subscription::{Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionStatus},
    uow::DynUnitOfWork,
    user::User,
};
use crate::in_transaction;
use chrono::{Days, Utc};
use tracing::info;
use uuid::Uuid;

pub struct PurchaseTariffCommand {
    uow: DynUnitOfWork,
}

impl PurchaseTariffCommand {
    pub fn new(uow: DynUnitOfWork) -> Self {
        Self { uow }
    }

    pub async fn execute(
        &self,
        user: &User,
        plan: SubscriptionPlan,
        devices: SubscriptionDevices,
    ) -> AppResult<Subscription> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;

        let price: Money = calculate_price(plan, devices, user)?;
        let kind = TransactionKind::Purchase;
        let order_id = format!("{}_{user_id}_{}", kind.as_str(), Uuid::new_v4().simple());

        in_transaction!(self.uow, |tx| {
            let balance = tx.transactions().balance_for_user(user_id).await?;
            if balance < price {
                return Err(DomainError::Billing(BillingError::InsufficientBalance).into());
            }

            let purchase_tx = Transaction::new_completed(user_id, price, kind, order_id.clone());
            tx.transactions().create(&purchase_tx).await?;

            let subscription =
                if let Some(mut sub) = tx.subscriptions().find_active_by_user_id(user_id).await? {
                    sub.renew(plan, devices)?;
                    tx.subscriptions().update(&sub).await?;
                    sub
                } else {
                    let sub = Subscription::new(
                        user_id,
                        plan,
                        Utc::now(),
                        Utc::now() + Days::new(plan.duration_days()),
                        SubscriptionStatus::Active,
                        devices,
                    );
                    tx.subscriptions().create(&sub).await?;
                    sub
                };

            info!(
                user_id = %user_id,
                order_id,
                plan = ?subscription.plan(),
                devices = %subscription.devices(),
                price = %price,
                expires_at = %subscription.expires_at(),
                "Тариф куплен"
            );

            Ok(subscription)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::AppError;
    use crate::domain::{
        DomainResult,
        billing::{DynTransactionRepository, TransactionId, TransactionRepository},
        subscription::{DynSubscriptionRepository, SubscriptionId, SubscriptionRepository},
        uow::{BoxedUowContext, UnitOfWork, UowContext},
        user::{DynUserRepository, ReferralCode, SubscriptionToken, TelegramId, UserId},
        vpn::{DynNodeRepository, DynVpnConnectionRepository},
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ==========================================
    // Моки
    // ==========================================

    struct MockSubscriptionRepository {
        active_sub: Option<Subscription>,
        created: Arc<Mutex<Vec<Subscription>>>,
        updated: Arc<Mutex<Vec<Subscription>>>,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, subscription: &Subscription) -> DomainResult<()> {
            self.created.lock().unwrap().push(subscription.clone());
            Ok(())
        }
        async fn update(&self, subscription: &Subscription) -> DomainResult<()> {
            self.updated.lock().unwrap().push(subscription.clone());
            Ok(())
        }
        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            Ok(self.active_sub.clone())
        }
        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_lapsed_active() не используется в этом юзкейсе")
        }
        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_due_for_expiry_warning() не используется в этом юзкейсе")
        }
    }

    struct MockTransactionRepository {
        balance: Money,
        created: Arc<Mutex<Vec<Transaction>>>,
    }

    #[async_trait]
    impl TransactionRepository for MockTransactionRepository {
        async fn create(&self, tx: &Transaction) -> DomainResult<TransactionId> {
            self.created.lock().unwrap().push(tx.clone());
            Ok(TransactionId::new(1))
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

    struct MockUowContext {
        sub_repo: Arc<MockSubscriptionRepository>,
        tx_repo: Arc<MockTransactionRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UowContext for MockUowContext {
        fn users(&self) -> DynUserRepository {
            unreachable!("users() не используется в этом юзкейсе")
        }
        fn subscriptions(&self) -> DynSubscriptionRepository {
            self.sub_repo.clone()
        }
        fn nodes(&self) -> DynNodeRepository {
            unreachable!("nodes() не используется в этом юзкейсе")
        }
        fn vpn_connections(&self) -> DynVpnConnectionRepository {
            unreachable!("vpn_connections() не используется в этом юзкейсе")
        }
        fn transactions(&self) -> DynTransactionRepository {
            self.tx_repo.clone()
        }
        async fn commit(&mut self) -> DomainResult<()> {
            *self.committed.lock().unwrap() = true;
            Ok(())
        }
        async fn rollback(&mut self) -> DomainResult<()> {
            *self.rolled_back.lock().unwrap() = true;
            Ok(())
        }
    }

    struct MockUnitOfWork {
        sub_repo: Arc<MockSubscriptionRepository>,
        tx_repo: Arc<MockTransactionRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
        begin_called: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UnitOfWork for MockUnitOfWork {
        async fn begin(&self) -> DomainResult<BoxedUowContext> {
            *self.begin_called.lock().unwrap() = true;
            Ok(Box::new(MockUowContext {
                sub_repo: self.sub_repo.clone(),
                tx_repo: self.tx_repo.clone(),
                committed: self.committed.clone(),
                rolled_back: self.rolled_back.clone(),
            }))
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn create_test_user(has_id: bool, base_price_kopecks: i64) -> User {
        let mut user = User::new(
            TelegramId::new(555),
            Uuid::new_v4(),
            Some("test".into()),
            "Test User".into(),
            Money::new(base_price_kopecks).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        if has_id {
            user.assign_id(UserId::new(1));
        }
        user
    }

    fn create_active_subscription(
        user_id: UserId,
        plan: SubscriptionPlan,
        devices: i32,
        days_left: u64,
        warning_sent: bool,
    ) -> Subscription {
        let now = Utc::now();
        Subscription::restore_from_db(
            SubscriptionId::new(10),
            user_id,
            plan,
            now - Days::new(1),
            now + Days::new(days_left),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(devices).unwrap(),
            warning_sent,
            now - Days::new(1),
        )
    }

    #[allow(clippy::type_complexity)]
    fn setup_command(
        active_sub: Option<Subscription>,
        balance: Money,
    ) -> (
        PurchaseTariffCommand,
        Arc<MockSubscriptionRepository>,
        Arc<MockTransactionRepository>,
        Arc<Mutex<bool>>,
        Arc<Mutex<bool>>,
        Arc<Mutex<bool>>,
    ) {
        let sub_repo = Arc::new(MockSubscriptionRepository {
            active_sub,
            created: Arc::new(Mutex::new(Vec::new())),
            updated: Arc::new(Mutex::new(Vec::new())),
        });
        let tx_repo = Arc::new(MockTransactionRepository {
            balance,
            created: Arc::new(Mutex::new(Vec::new())),
        });
        let committed = Arc::new(Mutex::new(false));
        let rolled_back = Arc::new(Mutex::new(false));
        let begin_called = Arc::new(Mutex::new(false));

        let uow = Arc::new(MockUnitOfWork {
            sub_repo: sub_repo.clone(),
            tx_repo: tx_repo.clone(),
            committed: committed.clone(),
            rolled_back: rolled_back.clone(),
            begin_called: begin_called.clone(),
        });

        (
            PurchaseTariffCommand::new(uow),
            sub_repo,
            tx_repo,
            committed,
            rolled_back,
            begin_called,
        )
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_creates_new_subscription_when_none_active() {
        let (cmd, sub_repo, tx_repo, committed, rolled_back, _begin) =
            setup_command(None, Money::new(1_000_000).unwrap());
        let user = create_test_user(true, 1000);
        let plan = SubscriptionPlan::Month1;
        let devices = SubscriptionDevices::new(3).unwrap();
        let expected_price = calculate_price(plan, devices, &user).unwrap();

        let result = cmd.execute(&user, plan, devices).await;
        assert!(result.is_ok());
        let sub = result.unwrap();

        assert_eq!(sub.plan(), plan);
        assert_eq!(sub.devices(), devices);
        assert_eq!(sub.status(), SubscriptionStatus::Active);

        assert_eq!(
            sub_repo.created.lock().unwrap().len(),
            1,
            "должна была создаться ровно одна подписка"
        );
        assert!(
            sub_repo.updated.lock().unwrap().is_empty(),
            "update() не должен вызываться при создании"
        );

        let created_txs = tx_repo.created.lock().unwrap();
        assert_eq!(created_txs.len(), 1);
        assert_eq!(created_txs[0].amount(), expected_price);
        assert_eq!(created_txs[0].kind(), TransactionKind::Purchase);

        assert!(*committed.lock().unwrap());
        assert!(!*rolled_back.lock().unwrap());
    }

    #[tokio::test]
    async fn test_extends_existing_active_subscription() {
        let user = create_test_user(true, 1000);
        let user_id = user.id().unwrap();
        let active_sub = create_active_subscription(user_id, SubscriptionPlan::Week1, 1, 3, true);

        let (cmd, sub_repo, tx_repo, committed, _rolled_back, _begin) =
            setup_command(Some(active_sub.clone()), Money::new(1_000_000).unwrap());

        let new_plan = SubscriptionPlan::Month3;
        let new_devices = SubscriptionDevices::new(5).unwrap();

        let result = cmd.execute(&user, new_plan, new_devices).await;
        assert!(result.is_ok());
        let sub = result.unwrap();

        assert_eq!(sub.plan(), new_plan, "план должен смениться на купленный");
        assert_eq!(
            sub.devices(),
            new_devices,
            "лимит устройств должен смениться на купленный"
        );
        assert_eq!(
            sub.expires_at(),
            active_sub.expires_at() + Days::new(new_plan.duration_days()),
            "срок должен УВЕЛИЧИТЬСЯ, а не замениться"
        );
        assert!(
            !sub.is_warning_sent(),
            "флаг предупреждения должен сброситься при продлении"
        );

        assert!(
            sub_repo.created.lock().unwrap().is_empty(),
            "create() не должен вызываться при продлении"
        );
        assert_eq!(sub_repo.updated.lock().unwrap().len(), 1);
        assert_eq!(tx_repo.created.lock().unwrap().len(), 1);
        assert!(*committed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_fails_when_balance_insufficient() {
        let (cmd, sub_repo, tx_repo, committed, rolled_back, begin_called) =
            setup_command(None, Money::new(0).unwrap());
        let user = create_test_user(true, 1000);

        let result = cmd
            .execute(
                &user,
                SubscriptionPlan::Month1,
                SubscriptionDevices::new(1).unwrap(),
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::Billing(BillingError::InsufficientBalance))
        ));
        assert!(
            *begin_called.lock().unwrap(),
            "транзакция должна была открыться перед проверкой баланса"
        );
        assert!(*rolled_back.lock().unwrap());
        assert!(!*committed.lock().unwrap());
        assert!(
            tx_repo.created.lock().unwrap().is_empty(),
            "транзакция списания не должна создаваться"
        );
        assert!(sub_repo.created.lock().unwrap().is_empty());
        assert!(sub_repo.updated.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fails_when_user_not_saved() {
        let (cmd, _sub_repo, _tx_repo, _committed, _rolled_back, begin_called) =
            setup_command(None, Money::new(1_000_000).unwrap());
        let user = create_test_user(false, 1000);

        let result = cmd
            .execute(
                &user,
                SubscriptionPlan::Month1,
                SubscriptionDevices::new(1).unwrap(),
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::User(UserError::EntityNotSaved))
        ));
        assert!(
            !*begin_called.lock().unwrap(),
            "транзакция не должна открываться, если юзер не сохранён"
        );
    }

    #[tokio::test]
    async fn test_fails_for_non_purchasable_plan() {
        let (cmd, _sub_repo, _tx_repo, _committed, _rolled_back, begin_called) =
            setup_command(None, Money::new(1_000_000).unwrap());
        let user = create_test_user(true, 1000);

        let result = cmd
            .execute(
                &user,
                SubscriptionPlan::Trial,
                SubscriptionDevices::new(1).unwrap(),
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::Billing(BillingError::PlanNotPurchasable(_)))
        ));
        assert!(
            !*begin_called.lock().unwrap(),
            "цена считается ДО открытия транзакции - Trial должен отсеяться без похода в БД"
        );
    }
}
