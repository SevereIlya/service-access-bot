use crate::application::{AppError, AppResult, Registration};
use crate::domain::{
    DomainError, ReferralError, UserError,
    billing::{Money, Transaction, TransactionKind},
    notification::DynNotifier,
    referral::{Referral, ReferralPolicy},
    uow::DynUnitOfWork,
    user::{ReferralCode, User},
};
use crate::in_transaction;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedReferral {
    pub welcome_bonus: Option<Money>,
}

pub struct ApplyReferralCommand {
    uow: DynUnitOfWork,
    notifier: DynNotifier,
    policy: ReferralPolicy,
}

impl ApplyReferralCommand {
    pub fn new(uow: DynUnitOfWork, notifier: DynNotifier, policy: ReferralPolicy) -> Self {
        Self {
            uow,
            notifier,
            policy,
        }
    }

    /// Привязывает нового пользователя к владельцу реферального кода и зачисляет ему welcome-бонус.
    /// Всё в одной транзакции.
    ///
    /// `Ok(None)` значит, что привязки не произошло - это ожидаемо:
    /// пользователь уже существовал, код неизвестен, самореферал или у него уже есть реферер.
    #[allow(clippy::similar_names)]
    pub async fn execute(
        &self,
        registration: &Registration,
        code: &ReferralCode,
    ) -> AppResult<Option<AppliedReferral>> {
        let Registration::Created(referred) = registration else {
            return Ok(None);
        };
        let referred_id = referred.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;
        let bonus = self.policy.welcome_bonus();

        let committed: AppResult<Option<User>> = in_transaction!(self.uow, |tx| {
            let Some(referrer) = tx.users().find_by_referral_code(code).await? else {
                return Ok(None);
            };
            let referrer_id = referrer.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;

            let referral = Referral::new(referrer_id, referred_id)?;
            tx.referrals().create(&referral).await?;

            if !bonus.is_zero() {
                let kind = TransactionKind::ReferralWelcomeBonus;
                let order_id = format!("{kind}_{referred_id}");
                let bonus_tx = Transaction::new_completed(referred_id, bonus, kind, order_id);
                tx.transactions().create(&bonus_tx).await?;
            }

            Ok(Some(referrer))
        });

        let referrer = match committed {
            Ok(Some(referrer)) => referrer,
            Ok(None) => {
                info!(referred_id = %referred_id, "Реферальный код не найден, привязка пропущена");
                return Ok(None);
            }
            Err(AppError::Domain(DomainError::Referral(ReferralError::AlreadyReferred))) => {
                info!(referred_id = %referred_id, "Реферал не привязан: уже есть реферер");
                return Ok(None);
            }
            Err(AppError::Domain(DomainError::Referral(ReferralError::SelfReferral))) => {
                warn!(referred_id = %referred_id, "Реферал не привязан: самореферал");
                return Ok(None);
            }
            Err(e) => return Err(e),
        };

        info!(
            referrer_id = ?referrer.id(),
            referred_id = %referred_id,
            welcome_bonus = %bonus,
            "Реферал привязан"
        );

        if let Err(e) = self
            .notifier
            .notify_referral_joined(&referrer, referred.public_identity())
            .await
        {
            warn!(
                error = ?e,
                referrer_id = ?referrer.id(),
                "Не удалось уведомить реферера о новом реферале"
            );
        }

        Ok(Some(AppliedReferral {
            welcome_bonus: (!bonus.is_zero()).then_some(bonus),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        billing::{DynTransactionRepository, TransactionId, TransactionRepository},
        device::{DeviceName, DeviceSetupCode},
        notification::Notifier,
        referral::{CashbackPercent, DynReferralRepository, ReferralId, ReferralRepository},
        subscription::DynSubscriptionRepository,
        uow::{BoxedUowContext, UnitOfWork, UowContext},
        user::{
            DynUserRepository, PublicIdentity, SubscriptionToken, TelegramId, UserId,
            UserRepository,
        },
        vpn::{DynNodeRepository, DynVpnConnectionRepository},
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // ==========================================
    // Моки
    // ==========================================

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
        async fn find_by_user_id(&self, _user_id: UserId) -> DomainResult<Option<User>> {
            unreachable!("find_by_user_id() не используется в этом юзкейсе")
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
            Ok(self.referrer.clone())
        }
    }

    struct MockReferralRepository {
        already_referred: bool,
        created: Arc<Mutex<Vec<Referral>>>,
    }

    #[async_trait]
    impl ReferralRepository for MockReferralRepository {
        async fn create(&self, referral: &Referral) -> DomainResult<ReferralId> {
            if self.already_referred {
                return Err(DomainError::Referral(ReferralError::AlreadyReferred));
            }
            self.created.lock().unwrap().push(referral.clone());
            Ok(ReferralId::new(1))
        }
        async fn find_by_referred_id(
            &self,
            _referred_id: UserId,
        ) -> DomainResult<Option<Referral>> {
            unreachable!("find_by_referred_id() не используется в этом юзкейсе")
        }
        async fn count_by_referrer(&self, _referrer_id: UserId) -> DomainResult<i64> {
            unreachable!("count_by_referrer() не используется в этом юзкейсе")
        }
    }

    struct MockTransactionRepository {
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
            unreachable!("balance_for_user() не используется в этом юзкейсе")
        }
        async fn sum_for_user_by_kind(
            &self,
            _user_id: UserId,
            _kind: TransactionKind,
        ) -> DomainResult<Money> {
            unreachable!("sum_for_user_by_kind() не используется в этом юзкейсе")
        }
    }

    struct MockUowContext {
        users: Arc<MockUserRepository>,
        referrals: Arc<MockReferralRepository>,
        transactions: Arc<MockTransactionRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UowContext for MockUowContext {
        fn users(&self) -> DynUserRepository {
            self.users.clone()
        }
        fn subscriptions(&self) -> DynSubscriptionRepository {
            unreachable!("subscriptions() не используется в этом юзкейсе")
        }
        fn nodes(&self) -> DynNodeRepository {
            unreachable!("nodes() не используется в этом юзкейсе")
        }
        fn vpn_connections(&self) -> DynVpnConnectionRepository {
            unreachable!("vpn_connections() не используется в этом юзкейсе")
        }
        fn transactions(&self) -> DynTransactionRepository {
            self.transactions.clone()
        }
        fn referrals(&self) -> DynReferralRepository {
            self.referrals.clone()
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
        users: Arc<MockUserRepository>,
        referrals: Arc<MockReferralRepository>,
        transactions: Arc<MockTransactionRepository>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UnitOfWork for MockUnitOfWork {
        async fn begin(&self) -> DomainResult<BoxedUowContext> {
            Ok(Box::new(MockUowContext {
                users: self.users.clone(),
                referrals: self.referrals.clone(),
                transactions: self.transactions.clone(),
                committed: self.committed.clone(),
                rolled_back: self.rolled_back.clone(),
            }))
        }
    }

    #[allow(clippy::type_complexity)]
    struct MockNotifier {
        fails: bool,
        joined: Arc<Mutex<Vec<(Option<UserId>, Option<PublicIdentity>)>>>,
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
            referrer: &User,
            friend: Option<PublicIdentity>,
        ) -> DomainResult<()> {
            self.joined.lock().unwrap().push((referrer.id(), friend));
            if self.fails {
                Err(DomainError::SystemFailure("telegram недоступен".into()))
            } else {
                Ok(())
            }
        }
        async fn notify_referral_cashback(
            &self,
            _referrer: &User,
            _friend: Option<PublicIdentity>,
            _amount: Money,
        ) -> DomainResult<()> {
            unreachable!("notify_referral_cashback() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    /// Описание сценария.
    struct Scenario {
        referrer: Option<User>,
        already_referred: bool,
        notifier_fails: bool,
        bonus_kopecks: i64,
    }

    /// Что команда сделала с БД и Телеграм
    #[allow(clippy::type_complexity)]
    struct Spies {
        referrals: Arc<Mutex<Vec<Referral>>>,
        transactions: Arc<Mutex<Vec<Transaction>>>,
        joined: Arc<Mutex<Vec<(Option<UserId>, Option<PublicIdentity>)>>>,
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    fn make_user(id: Option<i64>, code: &str) -> User {
        let n = id.unwrap_or(0);
        let mut user = User::new(
            TelegramId::new(1000 + n),
            Uuid::new_v4(),
            Some(format!("user{n}")),
            format!("User {n}"),
            Money::new(1_000).unwrap(),
            ReferralCode::new(code.into()),
            SubscriptionToken::new(format!("TOK{n}")),
        );
        if let Some(id) = id {
            user.assign_id(UserId::new(id));
        }
        user
    }

    fn happy() -> Scenario {
        Scenario {
            referrer: Some(make_user(Some(1), "REF1")),
            already_referred: false,
            notifier_fails: false,
            bonus_kopecks: 3_800,
        }
    }

    fn setup(scenario: Scenario) -> (ApplyReferralCommand, Spies) {
        let spies = Spies {
            referrals: Arc::new(Mutex::new(Vec::new())),
            transactions: Arc::new(Mutex::new(Vec::new())),
            joined: Arc::new(Mutex::new(Vec::new())),
            committed: Arc::new(Mutex::new(false)),
            rolled_back: Arc::new(Mutex::new(false)),
        };

        let uow = Arc::new(MockUnitOfWork {
            users: Arc::new(MockUserRepository {
                referrer: scenario.referrer,
            }),
            referrals: Arc::new(MockReferralRepository {
                already_referred: scenario.already_referred,
                created: spies.referrals.clone(),
            }),
            transactions: Arc::new(MockTransactionRepository {
                created: spies.transactions.clone(),
            }),
            committed: spies.committed.clone(),
            rolled_back: spies.rolled_back.clone(),
        });
        let notifier = Arc::new(MockNotifier {
            fails: scenario.notifier_fails,
            joined: spies.joined.clone(),
        });
        let policy = ReferralPolicy::new(
            Money::new(scenario.bonus_kopecks).unwrap(),
            CashbackPercent::new(10).unwrap(),
        );

        (ApplyReferralCommand::new(uow, notifier, policy), spies)
    }

    fn code() -> ReferralCode {
        ReferralCode::new("REF1".into())
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_applies_referral_and_credits_welcome_bonus() {
        let (cmd, spies) = setup(happy());
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(
            applied,
            Some(AppliedReferral {
                welcome_bonus: Some(Money::new(3_800).unwrap())
            })
        );

        let referrals = spies.referrals.lock().unwrap();
        assert_eq!(referrals.len(), 1);
        assert_eq!(referrals[0].referrer_id(), UserId::new(1));
        assert_eq!(referrals[0].referred_id(), UserId::new(2));

        let transactions = spies.transactions.lock().unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(
            transactions[0].user_id(),
            UserId::new(2),
            "бонус получает приглашённый"
        );
        assert_eq!(transactions[0].amount(), Money::new(3_800).unwrap());
        assert_eq!(
            transactions[0].kind(),
            TransactionKind::ReferralWelcomeBonus
        );
        assert_eq!(transactions[0].order_id(), "referral_welcome_bonus_2");

        assert!(*spies.committed.lock().unwrap());
        assert!(!*spies.rolled_back.lock().unwrap());

        let joined = spies.joined.lock().unwrap();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].0, Some(UserId::new(1)), "уведомляем реферера");
        assert_eq!(joined[0].1, Some(PublicIdentity::Username("user2".into())));
    }

    #[tokio::test]
    async fn test_zero_bonus_still_links_but_credits_nothing() {
        let (cmd, spies) = setup(Scenario {
            bonus_kopecks: 0,
            ..happy()
        });
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(
            applied,
            Some(AppliedReferral {
                welcome_bonus: None
            })
        );
        assert_eq!(
            spies.referrals.lock().unwrap().len(),
            1,
            "связь создаётся и без бонуса"
        );
        assert!(
            spies.transactions.lock().unwrap().is_empty(),
            "нулевой бонус не пишем: CHECK (amount > 0)"
        );
        assert_eq!(spies.joined.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_unknown_code_is_a_quiet_noop() {
        let (cmd, spies) = setup(Scenario {
            referrer: None,
            ..happy()
        });
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(applied, None);
        assert!(spies.referrals.lock().unwrap().is_empty());
        assert!(spies.transactions.lock().unwrap().is_empty());
        assert!(spies.joined.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_already_referred_rolls_back_and_stays_silent() {
        let (cmd, spies) = setup(Scenario {
            already_referred: true,
            ..happy()
        });
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(applied, None);
        assert!(*spies.rolled_back.lock().unwrap());
        assert!(!*spies.committed.lock().unwrap());
        assert!(
            spies.transactions.lock().unwrap().is_empty(),
            "бонуса быть не должно"
        );
        assert!(spies.joined.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_existing_user_never_opens_a_transaction() {
        let (cmd, spies) = setup(happy());
        let registration = Registration::Existing(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(applied, None);
        assert!(!*spies.committed.lock().unwrap());
        assert!(
            !*spies.rolled_back.lock().unwrap(),
            "UoW не должна была даже открыться"
        );
        assert!(spies.referrals.lock().unwrap().is_empty());
        assert!(spies.transactions.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_notifier_failure_does_not_undo_the_referral() {
        let (cmd, spies) = setup(Scenario {
            notifier_fails: true,
            ..happy()
        });
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert!(
            applied.is_some(),
            "сбой Telegram не должен превращать успех в ошибку"
        );
        assert!(*spies.committed.lock().unwrap());
        assert_eq!(
            spies.joined.lock().unwrap().len(),
            1,
            "попытка уведомить была"
        );
    }

    #[tokio::test]
    async fn test_self_referral_is_rolled_back() {
        let (cmd, spies) = setup(Scenario {
            referrer: Some(make_user(Some(2), "REF2")),
            ..happy()
        });
        let registration = Registration::Created(make_user(Some(2), "REF2"));

        let applied = cmd.execute(&registration, &code()).await.unwrap();

        assert_eq!(applied, None);
        assert!(*spies.rolled_back.lock().unwrap());
        assert!(spies.referrals.lock().unwrap().is_empty());
        assert!(spies.joined.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_unsaved_user_fails_before_touching_anything() {
        let (cmd, spies) = setup(happy());
        let registration = Registration::Created(make_user(None, "REF2"));

        let err = cmd.execute(&registration, &code()).await.unwrap_err();

        assert!(matches!(
            err,
            AppError::Domain(DomainError::User(UserError::EntityNotSaved))
        ));
        assert!(!*spies.committed.lock().unwrap());
        assert!(!*spies.rolled_back.lock().unwrap());
    }
}
