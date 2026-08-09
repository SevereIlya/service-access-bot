use crate::application::error::AppResult;
use crate::domain::error::DomainError;
use crate::domain::subscription::{
    Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionStatus,
};
use crate::domain::uow::{BoxedUowContext, DynUnitOfWork};
use crate::domain::user::User;
use chrono::{Days, Utc};

pub struct StartTrialCommand {
    uow: DynUnitOfWork,
}

impl StartTrialCommand {
    pub fn new(uow: DynUnitOfWork) -> Self {
        Self { uow }
    }

    pub async fn execute(&self, mut user: User) -> AppResult<Subscription> {
        let id = user.id().ok_or(DomainError::EntityNotSaved)?;

        let mut tx: BoxedUowContext = self.uow.begin().await?;

        if tx.subscriptions().find_active_by_user_id(id).await?.is_some() {
            tx.rollback().await?;
            return Err(DomainError::AlreadyHasSubscription.into());
        }

        user.use_trial()?;

        let sub = Subscription::new(
            id,
            SubscriptionPlan::Trial,
            Utc::now(),
            Utc::now() + Days::new(5),
            SubscriptionStatus::Active,
            SubscriptionDevices(2),
        );

        tx.subscriptions().create(&sub).await?;
        tx.users().update(&user).await?;

        tx.commit().await?;

        tracing::info!(
            user_id = %id,
            plan = ?sub.plan(),
            expires_at = %sub.expires_at(),
            "Пользователю успешно выдан Trial"
        );

        Ok(sub)
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::error::AppError;
    use crate::domain::error::DomainResult;
    use crate::domain::subscription::{
        DynSubscriptionRepository, SubscriptionRepository,
    };
    use crate::domain::uow::{UnitOfWork, UowContext};
    use crate::domain::user::{
        DynUserRepository, Money, ReferralCode, SubscriptionToken, TelegramId, UserId,
        UserRepository,
    };
    use async_trait::async_trait;
    use chrono::Months;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;
    // ==========================================
    // МОКИ
    // ==========================================

    struct MockUserRepository {
        updated_users: Arc<Mutex<Vec<User>>>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            Ok(UserId(1))
        }
        async fn update(&self, user: &User) -> DomainResult<()> {
            self.updated_users.lock().unwrap().push(user.clone());
            Ok(())
        }
        async fn find_by_user_id(&self, _user_id: UserId) -> DomainResult<Option<User>> {
            Ok(None)
        }
        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            Ok(None)
        }
    }

    struct MockSubscriptionRepository {
        has_active_sub: bool,
        created_subscriptions: Arc<Mutex<Vec<Subscription>>>,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, subscription: &Subscription) -> DomainResult<()> {
            self.created_subscriptions.lock().unwrap().push(subscription.clone());
            Ok(())
        }
        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            if self.has_active_sub {
                let sub = Subscription::new(
                    UserId(12),
                    SubscriptionPlan::Month3,
                    Utc::now(),
                    Utc::now() + Months::new(3),
                    SubscriptionStatus::Active,
                    SubscriptionDevices(2),
                );
                Ok(Some(sub))
            } else {
                Ok(None)
            }
        }
    }

    // =========================================

    struct MockUowContext {
        user_repo: Arc<MockUserRepository>,
        sub_repo: Arc<MockSubscriptionRepository>,
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
        committed: Arc<Mutex<bool>>,
        rolled_back: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl UnitOfWork for MockUnitOfWork {
        async fn begin(&self) -> DomainResult<BoxedUowContext> {
            Ok(Box::new(MockUowContext {
                user_repo: self.user_repo.clone(),
                sub_repo: self.sub_repo.clone(),
                committed: self.committed.clone(),
                rolled_back: self.rolled_back.clone(),
            }))
        }
    }

    // ==========================================
    // ВСПОМОГАТЕЛЬНАЯ ФУНКЦИЯ
    // ==========================================

    fn create_test_user(has_id: bool, trial_used: bool) -> User {
        let mut user = User::new(
            TelegramId(123),
            Uuid::new_v4(),
            Some("freddie".into()),
            "Freddie Mercury".into(),
            Money(15000),
            ReferralCode("REF".into()),
            SubscriptionToken("TOK".into()),
        );
        if has_id {
            user.assign_id(UserId(1));
        }
        if trial_used {
            let _ = user.use_trial();
        }
        user
    }

    fn setup_command(
        has_active_sub: bool,
    ) -> (
        StartTrialCommand,
        Arc<MockUserRepository>,
        Arc<MockSubscriptionRepository>,
        Arc<Mutex<bool>>,
        Arc<Mutex<bool>>,
    ) {
        let user_repo = Arc::new(MockUserRepository {
            updated_users: Arc::new(Mutex::new(Vec::new())),
        });
        let sub_repo = Arc::new(MockSubscriptionRepository {
            has_active_sub,
            created_subscriptions: Arc::new(Mutex::new(Vec::new())),
        });
        let committed = Arc::new(Mutex::new(false));
        let rolled_back = Arc::new(Mutex::new(false));

        let mock_uow = Arc::new(MockUnitOfWork {
            user_repo: user_repo.clone(),
            sub_repo: sub_repo.clone(),
            committed: committed.clone(),
            rolled_back: rolled_back.clone(),
        });

        (
            StartTrialCommand::new(mock_uow),
            user_repo,
            sub_repo,
            committed,
            rolled_back,
        )
    }

    // ==========================================
    // ТЕСТЫ
    // ==========================================

    #[tokio::test]
    async fn test_start_trial_success() {
        let (cmd, user_repo, sub_repo, committed, rolled_back) = setup_command(false);
        let user = create_test_user(true, false);
        let result = cmd.execute(user).await;

        assert!(result.is_ok(), "Ожидали успешную выдачу триала");

        let subscription = result.unwrap();

        assert_eq!(
            subscription.plan(),
            SubscriptionPlan::Trial,
            "План должен быть Trial"
        );
        assert_eq!(
            subscription.devices(),
            SubscriptionDevices(2),
            "Должно быть 2 устройства"
        );
        assert_eq!(
            subscription.status(),
            SubscriptionStatus::Active,
            "Статус должен быть Active"
        );

        let updated_users = user_repo.updated_users.lock().unwrap();

        assert_eq!(
            updated_users.len(),
            1,
            "Пользователь должен быть обновлён один раз"
        );
        assert!(
            updated_users[0].trial_used(),
            "Флаг trial_used должен быть установлен"
        );

        let created_subs = sub_repo.created_subscriptions.lock().unwrap();

        assert_eq!(
            created_subs.len(),
            1,
            "Подписка должна быть создана один раз"
        );
        assert!(
            *committed.lock().unwrap(),
            "Транзакция должна быть закоммичена"
        );
        assert!(
            !*rolled_back.lock().unwrap(),
            "Транзакция не должна быть откачена"
        );
    }

    #[tokio::test]
    async fn test_fails_if_user_not_saved_in_db() {
        let (cmd, _user_repo, _sub_repo, committed, _rolled_back) = setup_command(false);
        let user = create_test_user(false, false);
        let result = cmd.execute(user).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::EntityNotSaved)
        ));

        assert!(
            !*committed.lock().unwrap(),
            "Транзакция не должна быть закоммичена при ошибке"
        );
    }

    #[tokio::test]
    async fn test_fails_if_trial_already_used() {
        let (cmd, _user_repo, _sub_repo, committed, _rolled_back) = setup_command(false);
        let user = create_test_user(true, true);
        let result = cmd.execute(user).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::TrialAlreadyUsed)
        ));
        assert!(
            !*committed.lock().unwrap(),
            "Транзакция не должна быть закоммичена при ошибке"
        );
    }

    #[tokio::test]
    async fn test_fails_if_user_has_active_subscription() {
        let (cmd, _user_repo, _sub_repo, committed, rolled_back) = setup_command(true);
        let user = create_test_user(true, false);
        let result = cmd.execute(user).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::Domain(DomainError::AlreadyHasSubscription)
        ));
        assert!(
            *rolled_back.lock().unwrap(),
            "rollback() должен быть вызван при наличии активной подписки"
        );
        assert!(
            !*committed.lock().unwrap(),
            "Транзакция не должна быть закоммичена при ошибке"
        );
    }
}
