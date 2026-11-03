use crate::application::AppResult;
use crate::domain::{
    BillingError, DomainError, UserError,
    billing::{Money, Transaction, TransactionKind, calculate_price},
    notification::DynNotifier,
    referral::ReferralPolicy,
    subscription::{Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionStatus},
    uow::{DynUnitOfWork, UowContext},
    user::{User, UserId},
};
use crate::in_transaction;
use chrono::{Days, Utc};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Кэшбек, начисленный рефереру в транзакции покупки.
struct CashbackGrant {
    referrer: User,
    amount: Money,
}

/// Всё, что покупка выносит из транзакции наружу.
struct PurchaseOutcome {
    subscription: Subscription,
    cashback: Option<CashbackGrant>,
}

pub struct PurchaseTariffCommand {
    uow: DynUnitOfWork,
    notifier: DynNotifier,
    policy: ReferralPolicy,
}

impl PurchaseTariffCommand {
    pub fn new(uow: DynUnitOfWork, notifier: DynNotifier, policy: ReferralPolicy) -> Self {
        Self {
            uow,
            notifier,
            policy,
        }
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
        let order_id = format!("{kind}_{user_id}_{}", Uuid::new_v4().simple());

        let committed: AppResult<PurchaseOutcome> = in_transaction!(self.uow, |tx| {
            let balance = tx.transactions().balance_for_user(user_id).await?;
            if balance < price {
                return Err(DomainError::Billing(BillingError::InsufficientBalance {
                    required: price,
                    available: balance,
                })
                .into());
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

            let cashback = self.grant_cashback(tx.as_ref(), user_id, price, &order_id).await?;

            Ok(PurchaseOutcome {
                subscription,
                cashback,
            })
        });
        let PurchaseOutcome {
            subscription,
            cashback,
        } = committed?;

        info!(
            user_id = %user_id,
            order_id,
            plan = ?subscription.plan(),
            devices = %subscription.devices(),
            price = %price,
            expires_at = %subscription.expires_at(),
            "Тариф куплен"
        );

        if let Some(CashbackGrant { referrer, amount }) = cashback {
            info!(
                buyer_id = %user_id,
                referrer_id = ?referrer.id(),
                cashback = %amount,
                "Кэшбек начислен"
            );

            if let Err(e) = self
                .notifier
                .notify_referral_cashback(&referrer, user.public_identity(), amount)
                .await
            {
                warn!(
                    error = ?e,
                    referrer_id = ?referrer.id(),
                    "Не удалось уведомить реферера о кэшбеке"
                );
            }
        }

        Ok(subscription)
    }

    /// Начисляет рефереру покупателя кэшбек с этой покупки в той же транзакции.
    async fn grant_cashback(
        &self,
        tx: &dyn UowContext,
        buyer_id: UserId,
        price: Money,
        purchase_order_id: &str,
    ) -> AppResult<Option<CashbackGrant>> {
        let amount = self.policy.cashback_for(price);
        if amount.is_zero() {
            return Ok(None);
        }

        let Some(referral) = tx.referrals().find_by_referred_id(buyer_id).await? else {
            return Ok(None);
        };

        let referrer_id = referral.referrer_id();
        let Some(referrer) = tx.users().find_by_user_id(referrer_id).await? else {
            error!(
                buyer_id = %buyer_id,
                referrer_id = %referrer_id,
                "Есть связь реферала, но реферера нет в users: кэшбек пропущен, покупка проведена"
            );
            return Ok(None);
        };

        let kind = TransactionKind::ReferralCashback;
        let order_id = format!("{kind}_{purchase_order_id}");
        let cashback_tx = Transaction::new_completed(referrer_id, amount, kind, order_id);
        tx.transactions().create(&cashback_tx).await?;

        Ok(Some(CashbackGrant { referrer, amount }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::AppError;
    use crate::domain::{
        DomainResult,
        billing::{
            DynTransactionRepository, TransactionId, TransactionRepository, TransactionStatus,
        },
        device::{DeviceName, DeviceSetupCode},
        notification::Notifier,
        referral::{
            CashbackPercent, DynReferralRepository, Referral, ReferralId, ReferralRepository,
        },
        subscription::{DynSubscriptionRepository, SubscriptionId, SubscriptionRepository},
        uow::{BoxedUowContext, UnitOfWork},
        user::{
            DynUserRepository, PublicIdentity, ReferralCode, SubscriptionToken, TelegramId,
            UserRepository,
        },
        vpn::{DynNodeRepository, DynVpnConnectionRepository},
    };
    use async_trait::async_trait;
    use chrono::DateTime;
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
        /// Вид транзакции, на котором `create` падает (эмуляция сбоя БД).
        fail_on: Option<TransactionKind>,
        created: Arc<Mutex<Vec<Transaction>>>,
    }

    #[async_trait]
    impl TransactionRepository for MockTransactionRepository {
        async fn create(&self, tx: &Transaction) -> DomainResult<TransactionId> {
            if self.fail_on == Some(tx.kind()) {
                return Err(DomainError::SystemFailure("БД недоступна".into()));
            }
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
        async fn sum_for_user_by_kind(
            &self,
            _user_id: UserId,
            _kind: TransactionKind,
        ) -> DomainResult<Money> {
            unreachable!("sum_for_user_by_kind() не используется в этом юзкейсе")
        }
    }

    struct MockUserRepository {
        referrer: Option<User>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _user: &User) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }
        /// Отдаёт реферера только на запрос по ЕГО id: перепутанный id даст `None`, и тест на кэшбек упадёт.
        async fn find_by_user_id(&self, user_id: UserId) -> DomainResult<Option<User>> {
            Ok(self.referrer.clone().filter(|referrer| referrer.id() == Some(user_id)))
        }
        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_telegram_id() не используется в этом юзкейсе")
        }
        async fn find_by_subscription_token(
            &self,
            _token: &SubscriptionToken,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_subscription_token() не используется в этом юзкейсе")
        }
        async fn find_by_referral_code(&self, _code: &ReferralCode) -> DomainResult<Option<User>> {
            unreachable!("find_by_referral_code() не используется в этом юзкейсе")
        }
    }

    struct MockReferralRepository {
        referral: Option<Referral>,
    }

    #[async_trait]
    impl ReferralRepository for MockReferralRepository {
        async fn create(&self, _referral: &Referral) -> DomainResult<ReferralId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn find_by_referred_id(&self, referred_id: UserId) -> DomainResult<Option<Referral>> {
            Ok(self.referral.clone().filter(|referral| referral.referred_id() == referred_id))
        }
        async fn count_by_referrer(&self, _referrer_id: UserId) -> DomainResult<i64> {
            unreachable!("count_by_referrer() не используется в этом юзкейсе")
        }
    }

    #[derive(Debug, PartialEq)]
    struct CashbackNotice {
        referrer_id: Option<UserId>,
        friend: Option<PublicIdentity>,
        amount: Money,
    }

    struct MockNotifier {
        fails: bool,
        cashback: Arc<Mutex<Vec<CashbackNotice>>>,
    }

    #[async_trait]
    impl Notifier for MockNotifier {
        async fn notify_subscription_expired(&self, _user: &User) -> DomainResult<()> {
            unreachable!("notify_subscription_expired() не используется в этом юзкейсе")
        }
        async fn notify_subscription_expiring(
            &self,
            _user: &User,
            _expires_at: DateTime<Utc>,
        ) -> DomainResult<()> {
            unreachable!("notify_subscription_expiring() не используется в этом юзкейсе")
        }
        async fn notify_balance_topped_up(
            &self,
            _user: &User,
            _amount: Money,
            _new_balance: Money,
        ) -> DomainResult<()> {
            unreachable!("notify_balance_topped_up() не используется в этом юзкейсе")
        }
        async fn notify_new_device_detected(
            &self,
            _user: &User,
            _device_name: &DeviceName,
        ) -> DomainResult<()> {
            unreachable!("notify_new_device_detected() не используется в этом юзкейсе")
        }
        async fn notify_device_setup_code(
            &self,
            _user: &User,
            _device_name: &DeviceName,
            _code: &DeviceSetupCode,
        ) -> DomainResult<()> {
            unreachable!("notify_device_setup_code() не используется в этом юзкейсе")
        }
        async fn notify_referral_joined(
            &self,
            _referrer: &User,
            _friend: Option<PublicIdentity>,
        ) -> DomainResult<()> {
            unreachable!("notify_referral_joined() не используется в этом юзкейсе")
        }
        async fn notify_referral_cashback(
            &self,
            referrer: &User,
            friend: Option<PublicIdentity>,
            amount: Money,
        ) -> DomainResult<()> {
            self.cashback.lock().unwrap().push(CashbackNotice {
                referrer_id: referrer.id(),
                friend,
                amount,
            });
            if self.fails {
                Err(DomainError::SystemFailure("telegram недоступен".into()))
            } else {
                Ok(())
            }
        }
    }

    struct MockUowContext {
        user_repo: Arc<MockUserRepository>,
        sub_repo: Arc<MockSubscriptionRepository>,
        tx_repo: Arc<MockTransactionRepository>,
        referral_repo: Arc<MockReferralRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UowContext for MockUowContext {
        fn users(&self) -> DynUserRepository {
            self.user_repo.clone()
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
        fn referrals(&self) -> DynReferralRepository {
            self.referral_repo.clone()
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
        user_repo: Arc<MockUserRepository>,
        sub_repo: Arc<MockSubscriptionRepository>,
        tx_repo: Arc<MockTransactionRepository>,
        referral_repo: Arc<MockReferralRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
        begin_called: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UnitOfWork for MockUnitOfWork {
        async fn begin(&self) -> DomainResult<BoxedUowContext> {
            *self.begin_called.lock().unwrap() = true;
            Ok(Box::new(MockUowContext {
                user_repo: self.user_repo.clone(),
                sub_repo: self.sub_repo.clone(),
                tx_repo: self.tx_repo.clone(),
                referral_repo: self.referral_repo.clone(),
                committed: self.committed.clone(),
                rolled_back: self.rolled_back.clone(),
            }))
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    /// Описание сценария.
    struct Scenario {
        active_sub: Option<Subscription>,
        balance: Money,
        referral: Option<Referral>,
        referrer: Option<User>,
        fail_on: Option<TransactionKind>,
        notifier_fails: bool,
        cashback_percent: i32,
    }

    struct Spies {
        created_subs: Arc<Mutex<Vec<Subscription>>>,
        updated_subs: Arc<Mutex<Vec<Subscription>>>,
        transactions: Arc<Mutex<Vec<Transaction>>>,
        cashback_notices: Arc<Mutex<Vec<CashbackNotice>>>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
        begin_called: Arc<Mutex<bool>>,
    }

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

    /// Реферер покупателя из `create_test_user(true, ..)` (id 1) - пользователь с id 7.
    fn create_referrer() -> User {
        let mut user = User::new(
            TelegramId::new(777),
            Uuid::new_v4(),
            Some("boss".into()),
            "Boss".into(),
            Money::new(1000).unwrap(),
            ReferralCode::new("BOSS".into()),
            SubscriptionToken::new("TOK_BOSS".into()),
        );
        user.assign_id(UserId::new(7));
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

    /// Покупатель без реферера, баланс с запасом, кэшбек 10%.
    fn plain() -> Scenario {
        Scenario {
            active_sub: None,
            balance: Money::new(1_000_000).unwrap(),
            referral: None,
            referrer: None,
            fail_on: None,
            notifier_fails: false,
            cashback_percent: 10,
        }
    }

    /// Покупателя (id 1) пригласил пользователь с id 7.
    fn referred() -> Scenario {
        Scenario {
            referral: Some(Referral::new(UserId::new(7), UserId::new(1)).unwrap()),
            referrer: Some(create_referrer()),
            ..plain()
        }
    }

    fn setup(scenario: Scenario) -> (PurchaseTariffCommand, Spies) {
        let spies = Spies {
            created_subs: Arc::new(Mutex::new(Vec::new())),
            updated_subs: Arc::new(Mutex::new(Vec::new())),
            transactions: Arc::new(Mutex::new(Vec::new())),
            cashback_notices: Arc::new(Mutex::new(Vec::new())),
            committed: Arc::new(Mutex::new(false)),
            rolled_back: Arc::new(Mutex::new(false)),
            begin_called: Arc::new(Mutex::new(false)),
        };

        let uow = Arc::new(MockUnitOfWork {
            user_repo: Arc::new(MockUserRepository {
                referrer: scenario.referrer,
            }),
            sub_repo: Arc::new(MockSubscriptionRepository {
                active_sub: scenario.active_sub,
                created: spies.created_subs.clone(),
                updated: spies.updated_subs.clone(),
            }),
            tx_repo: Arc::new(MockTransactionRepository {
                balance: scenario.balance,
                fail_on: scenario.fail_on,
                created: spies.transactions.clone(),
            }),
            referral_repo: Arc::new(MockReferralRepository {
                referral: scenario.referral,
            }),
            committed: spies.committed.clone(),
            rolled_back: spies.rolled_back.clone(),
            begin_called: spies.begin_called.clone(),
        });
        let notifier = Arc::new(MockNotifier {
            fails: scenario.notifier_fails,
            cashback: spies.cashback_notices.clone(),
        });
        let policy = ReferralPolicy::new(
            Money::new(3_800).unwrap(),
            CashbackPercent::new(scenario.cashback_percent).unwrap(),
        );

        (PurchaseTariffCommand::new(uow, notifier, policy), spies)
    }

    /// Типовая покупка: месяц на 3 устройства.
    async fn buy(cmd: &PurchaseTariffCommand, buyer: &User) -> AppResult<Subscription> {
        cmd.execute(
            buyer,
            SubscriptionPlan::Month1,
            SubscriptionDevices::new(3).unwrap(),
        )
        .await
    }

    fn find_kind(transactions: &[Transaction], kind: TransactionKind) -> &Transaction {
        transactions
            .iter()
            .find(|tx| tx.kind() == kind)
            .unwrap_or_else(|| panic!("нет транзакции вида {kind}"))
    }

    // ==========================================
    // Тесты: покупка
    // ==========================================

    #[tokio::test]
    async fn test_creates_new_subscription_when_none_active() {
        let (cmd, spies) = setup(plain());
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
            spies.created_subs.lock().unwrap().len(),
            1,
            "должна была создаться ровно одна подписка"
        );
        assert!(
            spies.updated_subs.lock().unwrap().is_empty(),
            "update() не должен вызываться при создании"
        );

        let created_txs = spies.transactions.lock().unwrap();
        assert_eq!(created_txs.len(), 1);
        assert_eq!(created_txs[0].amount(), expected_price);
        assert_eq!(created_txs[0].kind(), TransactionKind::Purchase);

        assert!(*spies.committed.lock().unwrap());
        assert!(!*spies.rolled_back.lock().unwrap());
    }

    #[tokio::test]
    async fn test_extends_existing_active_subscription() {
        let user = create_test_user(true, 1000);
        let user_id = user.id().unwrap();
        let active_sub = create_active_subscription(user_id, SubscriptionPlan::Week1, 1, 3, true);

        let (cmd, spies) = setup(Scenario {
            active_sub: Some(active_sub.clone()),
            ..plain()
        });

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
            spies.created_subs.lock().unwrap().is_empty(),
            "create() не должен вызываться при продлении"
        );
        assert_eq!(spies.updated_subs.lock().unwrap().len(), 1);
        assert_eq!(spies.transactions.lock().unwrap().len(), 1);
        assert!(*spies.committed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_fails_when_balance_insufficient() {
        // Покупатель с рефералом: при отказе по балансу реферер не должен получить ничего.
        let (cmd, spies) = setup(Scenario {
            balance: Money::new(0).unwrap(),
            ..referred()
        });
        let user = create_test_user(true, 1000);

        let result = cmd
            .execute(
                &user,
                SubscriptionPlan::Month1,
                SubscriptionDevices::new(1).unwrap(),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Domain(DomainError::Billing(BillingError::InsufficientBalance {
                required,
                available,
            })) => {
                assert_eq!(available, Money::new(0).unwrap());
                assert!(
                    required.inner() > 0,
                    "цена тарифа должна быть положительной"
                );
            }
            other => panic!("Ожидалась BillingError::InsufficientBalance, получено {other:?}"),
        }
        assert!(
            *spies.begin_called.lock().unwrap(),
            "транзакция должна была открыться перед проверкой баланса"
        );
        assert!(*spies.rolled_back.lock().unwrap());
        assert!(!*spies.committed.lock().unwrap());
        assert!(
            spies.transactions.lock().unwrap().is_empty(),
            "ни списания, ни кэшбека"
        );
        assert!(spies.created_subs.lock().unwrap().is_empty());
        assert!(spies.updated_subs.lock().unwrap().is_empty());
        assert!(
            spies.cashback_notices.lock().unwrap().is_empty(),
            "по несостоявшейся покупке реферера не уведомляем"
        );
    }

    #[tokio::test]
    async fn test_fails_when_user_not_saved() {
        let (cmd, spies) = setup(plain());
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
            !*spies.begin_called.lock().unwrap(),
            "транзакция не должна открываться, если юзер не сохранён"
        );
    }

    #[tokio::test]
    async fn test_fails_for_non_purchasable_plan() {
        let (cmd, spies) = setup(plain());
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
            !*spies.begin_called.lock().unwrap(),
            "цена считается ДО открытия транзакции - Trial должен отсеяться без похода в БД"
        );
    }

    // ==========================================
    // Тесты: кэшбек
    // ==========================================

    #[tokio::test]
    async fn test_referred_buyer_pays_cashback_to_referrer() {
        let (cmd, spies) = setup(referred());
        let buyer = create_test_user(true, 1000);
        let plan = SubscriptionPlan::Month1;
        let devices = SubscriptionDevices::new(3).unwrap();
        let price = calculate_price(plan, devices, &buyer).unwrap();

        cmd.execute(&buyer, plan, devices).await.unwrap();

        let transactions = spies.transactions.lock().unwrap().clone();
        assert_eq!(
            transactions.len(),
            2,
            "списание покупателя + кэшбек реферера"
        );

        let purchase = find_kind(&transactions, TransactionKind::Purchase);
        assert_eq!(purchase.user_id(), UserId::new(1));
        assert_eq!(purchase.amount(), price);

        let cashback = find_kind(&transactions, TransactionKind::ReferralCashback);
        assert_eq!(
            cashback.user_id(),
            UserId::new(7),
            "кэшбек получает реферер, а не покупатель"
        );
        assert_eq!(
            cashback.amount().inner(),
            price.inner() / 10,
            "10% от цены с округлением вниз"
        );
        assert_eq!(
            cashback.status(),
            TransactionStatus::Success,
            "иначе баланс его не засчитает"
        );
        assert_eq!(
            cashback.order_id(),
            format!("referral_cashback_{}", purchase.order_id()),
            "детерминированный order_id: UNIQUE защитит от двойного начисления"
        );

        assert!(*spies.committed.lock().unwrap());
        assert!(!*spies.rolled_back.lock().unwrap());
    }

    #[tokio::test]
    async fn test_cashback_notice_goes_to_referrer_and_names_the_buyer() {
        let (cmd, spies) = setup(referred());
        let buyer = create_test_user(true, 1000);
        let plan = SubscriptionPlan::Month1;
        let devices = SubscriptionDevices::new(3).unwrap();
        let price = calculate_price(plan, devices, &buyer).unwrap();

        cmd.execute(&buyer, plan, devices).await.unwrap();

        assert_eq!(
            *spies.cashback_notices.lock().unwrap(),
            vec![CashbackNotice {
                referrer_id: Some(UserId::new(7)),
                friend: Some(PublicIdentity::Username("test".into())),
                amount: Money::new(price.inner() / 10).unwrap(),
            }]
        );
    }

    #[tokio::test]
    async fn test_buyer_without_referrer_gets_no_cashback() {
        let (cmd, spies) = setup(plain());
        let buyer = create_test_user(true, 1000);

        buy(&cmd, &buyer).await.unwrap();

        assert_eq!(
            spies.transactions.lock().unwrap().len(),
            1,
            "только списание"
        );
        assert!(spies.cashback_notices.lock().unwrap().is_empty());
        assert!(*spies.committed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_zero_percent_disables_cashback() {
        let (cmd, spies) = setup(Scenario {
            cashback_percent: 0,
            ..referred()
        });
        let buyer = create_test_user(true, 1000);

        buy(&cmd, &buyer).await.unwrap();

        assert_eq!(
            spies.transactions.lock().unwrap().len(),
            1,
            "кэшбек выключен: только списание (и CHECK (amount > 0) не сработает)"
        );
        assert!(spies.cashback_notices.lock().unwrap().is_empty());
        assert!(*spies.committed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_notifier_failure_does_not_undo_the_purchase() {
        let (cmd, spies) = setup(Scenario {
            notifier_fails: true,
            ..referred()
        });
        let buyer = create_test_user(true, 1000);

        let result = buy(&cmd, &buyer).await;

        assert!(result.is_ok(), "сбой Telegram не должен ломать покупку");
        assert!(*spies.committed.lock().unwrap());
        assert!(!*spies.rolled_back.lock().unwrap());
        assert_eq!(
            spies.transactions.lock().unwrap().len(),
            2,
            "кэшбек уже в БД"
        );
        assert_eq!(
            spies.cashback_notices.lock().unwrap().len(),
            1,
            "попытка уведомить была"
        );
    }

    #[tokio::test]
    async fn test_missing_referrer_row_skips_cashback_but_keeps_purchase() {
        let (cmd, spies) = setup(Scenario {
            referrer: None,
            ..referred()
        });
        let buyer = create_test_user(true, 1000);

        let result = buy(&cmd, &buyer).await;

        assert!(result.is_ok(), "покупку клиента не рушим из-за рефералки");
        assert!(*spies.committed.lock().unwrap());
        assert_eq!(
            spies.transactions.lock().unwrap().len(),
            1,
            "кэшбека нет, только списание"
        );
        assert!(spies.cashback_notices.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_cashback_write_failure_rolls_back_the_whole_purchase() {
        let (cmd, spies) = setup(Scenario {
            fail_on: Some(TransactionKind::ReferralCashback),
            ..referred()
        });
        let buyer = create_test_user(true, 1000);

        let err = buy(&cmd, &buyer).await.unwrap_err();

        assert!(matches!(
            err,
            AppError::Domain(DomainError::SystemFailure(_))
        ));
        assert!(*spies.rolled_back.lock().unwrap());
        assert!(
            !*spies.committed.lock().unwrap(),
            "сбой кэшбека откатывает покупку: это осознанный компромисс"
        );
        assert!(
            spies.cashback_notices.lock().unwrap().is_empty(),
            "ничего не закоммичено - уведомлять не о чем"
        );
    }
}
