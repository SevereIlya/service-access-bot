use crate::application::error::AppResult;
use crate::domain::notification::DynNotifier;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::user::DynUserRepository;
use tracing::{error, info, warn};

#[derive(Debug, Default)]
pub struct WarningSummary {
    pub warned: usize,
    pub failed: usize,
}

pub struct SendExpiryWarningsCommand {
    subscription_repo: DynSubscriptionRepository,
    user_repo: DynUserRepository,
    notifier: DynNotifier,
}

impl SendExpiryWarningsCommand {
    pub fn new(
        subscription_repo: DynSubscriptionRepository,
        user_repo: DynUserRepository,
        notifier: DynNotifier,
    ) -> Self {
        Self {
            subscription_repo,
            user_repo,
            notifier,
        }
    }

    pub async fn execute(&self) -> AppResult<WarningSummary> {
        let expiring = self.subscription_repo.find_due_for_expiry_warning().await?;

        let mut summary = WarningSummary::default();

        for mut sub in expiring {
            let user_id = sub.user_id();

            let user = match self.user_repo.find_by_user_id(user_id).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    error!(user_id = %user_id, "юзер не найден");
                    summary.failed += 1;
                    continue;
                }
                Err(e) => {
                    error!(user_id = %user_id, error = %e, "ошибка получения юзера");
                    summary.failed += 1;
                    continue;
                }
            };

            if let Err(e) =
                self.notifier.notify_subscription_expiring(&user, sub.expires_at()).await
            {
                warn!(user_id = %user_id, error = %e, "не удалось отправить уведомление");
                summary.failed += 1;
                continue;
            }

            sub.mark_warning_sent();
            if let Err(e) = self.subscription_repo.update(&sub).await {
                error!(user_id = %user_id, error = %e, "сообщение отправлено, но не удалось обновить флаг в БД");
                summary.failed += 1;
                continue;
            }

            info!(user_id = %user_id, "предупреждение об истечении подписки успешно отправлено");

            summary.warned += 1;
        }

        Ok(summary)
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::{DomainError, DomainResult};
    use crate::domain::notification::Notifier;
    use crate::domain::subscription::{
        Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionRepository,
        SubscriptionStatus,
    };
    use crate::domain::user::{
        Money, ReferralCode, SubscriptionToken, TelegramId, User, UserId, UserRepository,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Days, Utc};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // ==========================================
    // Моки
    // ==========================================

    struct MockUserRepository {
        users: Vec<User>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _user: &User) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }
        async fn find_by_user_id(&self, user_id: UserId) -> DomainResult<Option<User>> {
            Ok(self.users.iter().find(|u| u.id() == Some(user_id)).cloned())
        }
        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            unreachable!("find_by_telegram_id() не используется в этом юзкейсе")
        }
    }

    struct MockSubscriptionRepository {
        expiring: Vec<Subscription>,
        updated_subs: Arc<Mutex<Vec<Subscription>>>,
        fail_update: bool,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, subscription: &Subscription) -> DomainResult<()> {
            if self.fail_update {
                return Err(DomainError::SystemFailure("DB error".into()));
            }
            self.updated_subs.lock().unwrap().push(subscription.clone());
            Ok(())
        }
        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            unreachable!("find_active_by_user_id() не используется в этом юзкейсе")
        }
        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_lapsed_active() не используется в этом юзкейсе")
        }
        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            Ok(self.expiring.clone())
        }
    }

    struct MockNotifier {
        fail: bool,
        calls: Arc<Mutex<Vec<(UserId, DateTime<Utc>)>>>,
    }

    #[async_trait]
    impl Notifier for MockNotifier {
        async fn notify_subscription_expired(&self, _user: &User) -> DomainResult<()> {
            unreachable!("notify_subscription_expired() не используется в этом юзкейсе")
        }
        async fn notify_subscription_expiring(
            &self,
            user: &User,
            expires_at: DateTime<Utc>,
        ) -> DomainResult<()> {
            self.calls.lock().unwrap().push((user.id().unwrap(), expires_at));
            if self.fail {
                return Err(DomainError::SystemFailure("notify failed (test)".into()));
            }
            Ok(())
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn make_user(id: i64) -> User {
        let mut user = User::new(
            TelegramId::new(2000 + id),
            Uuid::new_v4(),
            Some(format!("user{id}")),
            format!("User {id}"),
            Money::new(15000).unwrap(),
            ReferralCode::new(format!("REF{id}")),
            SubscriptionToken::new(format!("TOK{id}")),
        );
        user.assign_id(UserId::new(id));
        user
    }

    fn make_expiring_subscription(
        user_id: i64,
        expires_at: DateTime<Utc>,
    ) -> Subscription {
        Subscription::new(
            UserId::new(user_id),
            SubscriptionPlan::Month1,
            Utc::now() - Days::new(29),
            expires_at,
            SubscriptionStatus::Active,
            SubscriptionDevices::new(2).unwrap(),
        )
    }

    #[allow(clippy::type_complexity)]
    fn setup(
        users: Vec<User>,
        expiring: Vec<Subscription>,
        fail_notify: bool,
        fail_update: bool,
    ) -> (
        SendExpiryWarningsCommand,
        Arc<Mutex<Vec<Subscription>>>,
        Arc<Mutex<Vec<(UserId, DateTime<Utc>)>>>,
    ) {
        let user_repo = Arc::new(MockUserRepository { users });
        let updated_subs = Arc::new(Mutex::new(Vec::new()));
        let sub_repo = Arc::new(MockSubscriptionRepository {
            expiring,
            updated_subs: updated_subs.clone(),
            fail_update,
        });
        let calls = Arc::new(Mutex::new(Vec::new()));
        let notifier = Arc::new(MockNotifier {
            fail: fail_notify,
            calls: calls.clone(),
        });
        let cmd = SendExpiryWarningsCommand::new(sub_repo, user_repo, notifier);
        (cmd, updated_subs, calls)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_no_expiring_subscriptions_does_nothing() {
        let (cmd, updated, calls) = setup(vec![], vec![], false, false);
        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 0);
        assert_eq!(summary.failed, 0);
        assert!(calls.lock().unwrap().is_empty());
        assert!(updated.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sends_warning_and_updates_db_happy_path() {
        let expires_at = Utc::now() + Days::new(1);
        let user = make_user(1);
        let sub = make_expiring_subscription(1, expires_at);
        let (cmd, updated, calls) = setup(vec![user], vec![sub], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 1);
        assert_eq!(summary.failed, 0);

        let calls_guard = calls.lock().unwrap();

        assert_eq!(calls_guard.len(), 1);
        assert_eq!(calls_guard[0].0, UserId::new(1));

        let updated_guard = updated.lock().unwrap();

        assert_eq!(updated_guard.len(), 1);
        assert!(
            updated_guard[0].is_warning_sent(),
            "Флаг должен быть установлен"
        );
    }

    #[tokio::test]
    async fn test_user_not_found_skips_and_counts_as_failed() {
        let sub = make_expiring_subscription(1, Utc::now() + Days::new(1));
        let (cmd, updated, calls) = setup(vec![], vec![sub], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 0);
        assert_eq!(summary.failed, 1);
        assert!(calls.lock().unwrap().is_empty());
        assert!(
            updated.lock().unwrap().is_empty(),
            "БД не должна обновляться"
        );
    }

    #[tokio::test]
    async fn test_notify_failure_skips_db_update() {
        let user = make_user(1);
        let sub = make_expiring_subscription(1, Utc::now() + Days::new(1));

        let (cmd, updated, calls) = setup(vec![user], vec![sub], true, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 0);
        assert_eq!(summary.failed, 1);
        assert_eq!(calls.lock().unwrap().len(), 1, "Попытка отправки была");
        assert!(
            updated.lock().unwrap().is_empty(),
            "БД не должна обновляться при падении сети"
        );
    }

    #[tokio::test]
    async fn test_db_update_failure_counts_as_failed() {
        let user = make_user(1);
        let sub = make_expiring_subscription(1, Utc::now() + Days::new(1));

        let (cmd, _updated, calls) = setup(vec![user], vec![sub], false, true);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(
            summary.warned, 0,
            "Не засчитываем в успех, если стейт не сохранен"
        );
        assert_eq!(summary.failed, 1);
        assert_eq!(calls.lock().unwrap().len(), 1, "Телега отработала");
    }

    #[tokio::test]
    async fn test_processes_multiple_subscriptions_independently() {
        let user1 = make_user(1);
        let user2 = make_user(2);
        let sub1 = make_expiring_subscription(1, Utc::now() + Days::new(1));
        let sub2 = make_expiring_subscription(2, Utc::now() + Days::new(1));

        let (cmd, updated, calls) =
            setup(vec![user1, user2], vec![sub1, sub2], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(updated.lock().unwrap().len(), 2);
    }
}
