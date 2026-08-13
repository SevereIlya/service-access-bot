use crate::application::error::AppResult;
use crate::domain::error::DomainError;
use crate::domain::error::SubscriptionError::AlreadyHasActive;
use crate::domain::error::UserError::EntityNotSaved;
use crate::domain::subscription::{
    Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionStatus,
};
use crate::domain::uow::DynUnitOfWork;
use crate::domain::user::User;
use crate::in_transaction;
use chrono::{Days, Utc};

pub struct StartTrialCommand {
    uow: DynUnitOfWork,
}

impl StartTrialCommand {
    pub fn new(uow: DynUnitOfWork) -> Self {
        Self { uow }
    }

    pub async fn execute(&self, mut user: User) -> AppResult<Subscription> {
        let id = user.id().ok_or(DomainError::User(EntityNotSaved))?;

        in_transaction!(self.uow, |tx| {
            if tx.subscriptions().find_active_by_user_id(id).await?.is_some() {
                return Err(DomainError::Subscription(AlreadyHasActive).into());
            }

            user.use_trial()?;

            let sub = Subscription::new(
                id,
                SubscriptionPlan::Trial,
                Utc::now(),
                Utc::now() + Days::new(5),
                SubscriptionStatus::Active,
                SubscriptionDevices::new(2)?,
            );

            tx.subscriptions().create(&sub).await?;
            tx.users().update(&user).await?;

            tracing::info!(
                user_id = %id,
                plan = ?sub.plan(),
                expires_at = %sub.expires_at(),
                "Пользователю успешно выдан Trial"
            );

            Ok(sub)
        })
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
    use crate::domain::error::UserError::TrialAlreadyUsed;
    use crate::domain::subscription::{
        DynSubscriptionRepository, SubscriptionRepository,
    };
    use crate::domain::uow::{BoxedUowContext, UnitOfWork, UowContext};
    use crate::domain::user::{
        DynUserRepository, Money, ReferralCode, SubscriptionToken, TelegramId, UserId,
        UserRepository,
    };
    use async_trait::async_trait;
    use chrono::Months;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // ==========================================
    // Моки
    // ==========================================

    struct MockUserRepository {
        updated_users: Arc<Mutex<Vec<User>>>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            Ok(UserId::new(1))
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

        async fn update(&self, _subscription: &Subscription) -> DomainResult<()> {
            Ok(())
        }

        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            if self.has_active_sub {
                let sub = Subscription::new(
                    UserId::new(12),
                    SubscriptionPlan::Month3,
                    Utc::now(),
                    Utc::now() + Months::new(3),
                    SubscriptionStatus::Active,
                    SubscriptionDevices::new(2)?,
                );
                Ok(Some(sub))
            } else {
                Ok(None)
            }
        }

        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            Ok(vec![])
        }

        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            Ok(vec![])
        }
    }

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
    // Хелперы
    // ==========================================

    fn create_test_user(has_id: bool, trial_used: bool) -> User {
        let mut user = User::new(
            TelegramId::new(123),
            Uuid::new_v4(),
            Some("freddie".into()),
            "Freddie Mercury".into(),
            Money::new(15000).unwrap(),
            ReferralCode::new("REF".into()),
            SubscriptionToken::new("TOK".into()),
        );
        if has_id {
            user.assign_id(UserId::new(1));
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
    // Тесты
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
            SubscriptionDevices::new(2).unwrap(),
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
            AppError::Domain(DomainError::User(EntityNotSaved))
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
            AppError::Domain(DomainError::User(TrialAlreadyUsed))
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
            AppError::Domain(DomainError::Subscription(AlreadyHasActive))
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
