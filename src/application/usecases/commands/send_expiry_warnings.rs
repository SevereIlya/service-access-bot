use crate::application::error::AppResult;
use crate::domain::notification::DynNotifier;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::user::DynUserRepository;
use chrono::{Duration, Utc};
use tracing::{error, warn};

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
        let now = Utc::now();
        let window_start = now + Duration::hours(23);
        let window_end = now + Duration::hours(24);

        let expiring = self
            .subscription_repo
            .find_expiring_between(window_start, window_end)
            .await?;

        let mut summary = WarningSummary::default();

        for sub in expiring {
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
        seen_window: Arc<Mutex<Option<(DateTime<Utc>, DateTime<Utc>)>>>,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
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
        async fn find_expiring_between(
            &self,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
        ) -> DomainResult<Vec<Subscription>> {
            *self.seen_window.lock().unwrap() = Some((start, end));
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
    ) -> (
        SendExpiryWarningsCommand,
        Arc<Mutex<Option<(DateTime<Utc>, DateTime<Utc>)>>>,
        Arc<Mutex<Vec<(UserId, DateTime<Utc>)>>>,
    ) {
        let user_repo = Arc::new(MockUserRepository { users });
        let seen_window = Arc::new(Mutex::new(None));
        let sub_repo = Arc::new(MockSubscriptionRepository {
            expiring,
            seen_window: seen_window.clone(),
        });
        let calls = Arc::new(Mutex::new(Vec::new()));
        let notifier = Arc::new(MockNotifier {
            fail: fail_notify,
            calls: calls.clone(),
        });
        let cmd = SendExpiryWarningsCommand::new(sub_repo, user_repo, notifier);
        (cmd, seen_window, calls)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_no_expiring_subscriptions_does_nothing() {
        let (cmd, _window, calls) = setup(vec![], vec![], false);
        let summary = cmd.execute().await.unwrap();
        assert_eq!(summary.warned, 0);
        assert_eq!(summary.failed, 0);
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sends_warning_happy_path() {
        let expires_at = Utc::now() + Days::new(1);
        let user = make_user(1);
        let sub = make_expiring_subscription(1, expires_at);
        let (cmd, _window, calls) = setup(vec![user], vec![sub], false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 1);
        assert_eq!(summary.failed, 0);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, UserId::new(1));
        assert_eq!(
            calls[0].1, expires_at,
            "юзеру должна прийти точная дата истечения его подписки"
        );
    }

    #[tokio::test]
    async fn test_uses_23_to_24_hour_window() {
        let (cmd, window, _calls) = setup(vec![], vec![], false);
        let now_before = Utc::now();
        cmd.execute().await.unwrap();

        let (start, end) =
            window.lock().unwrap().expect("find_expiring_between должен был вызваться");
        let expected_start = now_before + chrono::Duration::hours(23);
        let expected_end = now_before + chrono::Duration::hours(24);

        assert!(
            (start - expected_start).num_milliseconds().abs() < 500,
            "начало окна должно быть ~23 часа от текущего момента"
        );
        assert!(
            (end - expected_end).num_milliseconds().abs() < 500,
            "конец окна должен быть ~24 часа от текущего момента"
        );
    }

    #[tokio::test]
    async fn test_user_not_found_counts_as_failed() {
        let sub = make_expiring_subscription(1, Utc::now() + Days::new(1));
        let (cmd, _window, calls) = setup(vec![], vec![sub], false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 0);
        assert_eq!(summary.failed, 1);
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_notify_failure_counts_as_failed() {
        let user = make_user(1);
        let sub = make_expiring_subscription(1, Utc::now() + Days::new(1));
        let (cmd, _window, calls) = setup(vec![user], vec![sub], true);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(
            summary.warned, 0,
            "тут уведомление - вся работа команды, поэтому её провал = провал операции"
        );
        assert_eq!(summary.failed, 1);
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "попытка отправить всё же была"
        );
    }

    #[tokio::test]
    async fn test_processes_multiple_subscriptions_independently() {
        let user1 = make_user(1);
        let user2 = make_user(2);
        let sub1 = make_expiring_subscription(1, Utc::now() + Days::new(1));
        let sub2 = make_expiring_subscription(2, Utc::now() + Days::new(1));
        let (cmd, _window, calls) = setup(vec![user1, user2], vec![sub1, sub2], false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.warned, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(calls.lock().unwrap().len(), 2);
    }
}
