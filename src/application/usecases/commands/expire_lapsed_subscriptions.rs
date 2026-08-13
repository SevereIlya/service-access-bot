use crate::application::error::AppResult;
use crate::domain::notification::DynNotifier;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::subscription::Subscription;
use crate::domain::user::DynUserRepository;
use crate::domain::vpn::DynVpnAccessRevoker;
use tracing::{error, info, warn};

#[derive(Debug, Default)]
pub struct ExpirySummary {
    pub expired: usize,
    pub failed: usize,
}

pub struct ExpireLapsedSubscriptionsCommand {
    subscription_repo: DynSubscriptionRepository,
    user_repo: DynUserRepository,
    vpn_revoker: DynVpnAccessRevoker,
    notifier: DynNotifier,
}

impl ExpireLapsedSubscriptionsCommand {
    pub fn new(
        subscription_repo: DynSubscriptionRepository,
        user_repo: DynUserRepository,
        vpn_revoker: DynVpnAccessRevoker,
        notifier: DynNotifier,
    ) -> Self {
        Self {
            subscription_repo,
            user_repo,
            vpn_revoker,
            notifier,
        }
    }

    pub async fn execute(&self) -> AppResult<ExpirySummary> {
        let lapsed: Vec<Subscription> =
            self.subscription_repo.find_lapsed_active().await?;
        let mut summary = ExpirySummary::default();

        for mut sub in lapsed {
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

            if let Err(e) = self.vpn_revoker.revoke_all(user_id).await {
                error!(user_id = %user_id, error = %e, "не удалось отключить VPN, повтор через час");
                summary.failed += 1;
                continue;
            }

            sub.expire();
            if let Err(e) = self.subscription_repo.update(&sub).await {
                error!(user_id = %user_id, error = %e, "не удалось обновить статус подписки");
                summary.failed += 1;
                continue;
            }

            if let Err(e) = self.notifier.notify_subscription_expired(&user).await {
                warn!(user_id = %user_id, error = %e, "не удалось отправить уведомление");
            }

            info!(user_id = %user_id, "подписка помечена как истёкшая");
            summary.expired += 1;
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
    use crate::domain::vpn::VpnAccessRevoker;
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
        lapsed: Vec<Subscription>,
        updated: Arc<Mutex<Vec<Subscription>>>,
        fail_update: bool,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, subscription: &Subscription) -> DomainResult<()> {
            if self.fail_update {
                return Err(DomainError::SystemFailure("update failed (test)".into()));
            }
            self.updated.lock().unwrap().push(subscription.clone());
            Ok(())
        }
        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            unreachable!("find_active_by_user_id() не используется в этом юзкейсе")
        }
        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            Ok(self.lapsed.clone())
        }
        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_due_for_expiry_warning() не используется в этом юзкейсе")
        }
    }

    struct MockVpnAccessRevoker {
        fail_for: Vec<UserId>,
        calls: Arc<Mutex<Vec<UserId>>>,
    }

    #[async_trait]
    impl VpnAccessRevoker for MockVpnAccessRevoker {
        async fn revoke_all(&self, user_id: UserId) -> DomainResult<()> {
            self.calls.lock().unwrap().push(user_id);
            if self.fail_for.contains(&user_id) {
                return Err(DomainError::SystemFailure(
                    "vpn revoke failed (test)".into(),
                ));
            }
            Ok(())
        }
    }
    struct MockNotifier {
        fail: bool,
        expired_calls: Arc<Mutex<Vec<UserId>>>,
    }

    #[async_trait]
    impl Notifier for MockNotifier {
        async fn notify_subscription_expired(&self, user: &User) -> DomainResult<()> {
            self.expired_calls.lock().unwrap().push(user.id().unwrap());
            if self.fail {
                return Err(DomainError::SystemFailure("notify failed (test)".into()));
            }
            Ok(())
        }
        async fn notify_subscription_expiring(
            &self,
            _user: &User,
            _expires_at: DateTime<Utc>,
        ) -> DomainResult<()> {
            unreachable!("notify_subscription_expiring() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn make_user(id: i64) -> User {
        let mut user = User::new(
            TelegramId::new(1000 + id),
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

    fn make_lapsed_subscription(user_id: i64) -> Subscription {
        let now = Utc::now();
        Subscription::new(
            UserId::new(user_id),
            SubscriptionPlan::Month1,
            now - Days::new(31),
            now - Days::new(1),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(2).unwrap(),
        )
    }

    #[allow(clippy::type_complexity)]
    fn setup(
        users: Vec<User>,
        lapsed: Vec<Subscription>,
        fail_vpn_for: Vec<UserId>,
        fail_db_update: bool,
        fail_notify: bool,
    ) -> (
        ExpireLapsedSubscriptionsCommand,
        Arc<Mutex<Vec<Subscription>>>,
        Arc<Mutex<Vec<UserId>>>,
        Arc<Mutex<Vec<UserId>>>,
    ) {
        let user_repo = Arc::new(MockUserRepository { users });
        let updated = Arc::new(Mutex::new(Vec::new()));
        let sub_repo = Arc::new(MockSubscriptionRepository {
            lapsed,
            updated: updated.clone(),
            fail_update: fail_db_update,
        });
        let vpn_calls = Arc::new(Mutex::new(Vec::new()));
        let vpn = Arc::new(MockVpnAccessRevoker {
            fail_for: fail_vpn_for,
            calls: vpn_calls.clone(),
        });
        let notify_calls = Arc::new(Mutex::new(Vec::new()));
        let notifier = Arc::new(MockNotifier {
            fail: fail_notify,
            expired_calls: notify_calls.clone(),
        });
        let cmd =
            ExpireLapsedSubscriptionsCommand::new(sub_repo, user_repo, vpn, notifier);
        (cmd, updated, vpn_calls, notify_calls)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_no_lapsed_subscriptions_does_nothing() {
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![], vec![], vec![], false, false);
        let summary = cmd.execute().await.unwrap();
        assert_eq!(summary.expired, 0);
        assert_eq!(summary.failed, 0);
        assert!(
            vpn_calls.lock().unwrap().is_empty(),
            "VPN не трогаем без должников"
        );
        assert!(notify_calls.lock().unwrap().is_empty());
        assert!(updated.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_expires_subscription_happy_path() {
        let user = make_user(1);
        let sub = make_lapsed_subscription(1);
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![user], vec![sub], vec![], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.expired, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(*vpn_calls.lock().unwrap(), vec![UserId::new(1)]);

        let updated_subs = updated.lock().unwrap();

        assert_eq!(updated_subs.len(), 1);
        assert_eq!(
            updated_subs[0].status(),
            SubscriptionStatus::Expired,
            "статус в БД должен смениться на Expired"
        );
        assert_eq!(*notify_calls.lock().unwrap(), vec![UserId::new(1)]);
    }

    #[tokio::test]
    async fn test_user_not_found_is_skipped_and_counted_as_failed() {
        let sub = make_lapsed_subscription(1); // юзера с id=1 в users нет
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![], vec![sub], vec![], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.expired, 0);
        assert_eq!(summary.failed, 1);
        assert!(
            vpn_calls.lock().unwrap().is_empty(),
            "нельзя трогать VPN, если юзер не найден"
        );
        assert!(updated.lock().unwrap().is_empty());
        assert!(notify_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vpn_revoke_failure_leaves_subscription_active_for_retry() {
        let user = make_user(1);
        let sub = make_lapsed_subscription(1);
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![user], vec![sub], vec![UserId::new(1)], false, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.expired, 0);
        assert_eq!(summary.failed, 1);
        assert_eq!(
            vpn_calls.lock().unwrap().len(),
            1,
            "попытка отключить VPN была"
        );
        assert!(
            updated.lock().unwrap().is_empty(),
            "если VPN не отключился - статус НЕ должен меняться, иначе юзер потеряет доступ без ретрая"
        );
        assert!(
            notify_calls.lock().unwrap().is_empty(),
            "уведомление не уходит без отключённого VPN"
        );
    }

    #[tokio::test]
    async fn test_db_update_failure_does_not_send_notification() {
        let user = make_user(1);
        let sub = make_lapsed_subscription(1);
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![user], vec![sub], vec![], true, false);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(summary.expired, 0);
        assert_eq!(summary.failed, 1);
        assert_eq!(
            vpn_calls.lock().unwrap().len(),
            1,
            "VPN уже успел отключиться к этому моменту"
        );
        assert!(updated.lock().unwrap().is_empty());
        assert!(
            notify_calls.lock().unwrap().is_empty(),
            "нет смысла уведомлять, если факт даже не зафиксирован в БД"
        );
    }

    #[tokio::test]
    async fn test_notify_failure_does_not_undo_already_completed_expiry() {
        let user = make_user(1);
        let sub = make_lapsed_subscription(1);
        let (cmd, updated, vpn_calls, notify_calls) =
            setup(vec![user], vec![sub], vec![], false, true);

        let summary = cmd.execute().await.unwrap();

        assert_eq!(
            summary.expired, 1,
            "VPN отключён и БД обновлена — сбой уведомления best-effort, не откатывает факт"
        );
        assert_eq!(summary.failed, 0);
        assert_eq!(vpn_calls.lock().unwrap().len(), 1);
        assert_eq!(updated.lock().unwrap().len(), 1);
        assert_eq!(
            notify_calls.lock().unwrap().len(),
            1,
            "попытка уведомить всё же случилась"
        );
    }

    #[tokio::test]
    async fn test_processes_multiple_subscriptions_independently() {
        // юзер 1 - VPN не отключается, юзер 2 - всё ок
        let user1 = make_user(1);
        let user2 = make_user(2);
        let sub1 = make_lapsed_subscription(1);
        let sub2 = make_lapsed_subscription(2);

        let (cmd, updated, vpn_calls, notify_calls) = setup(
            vec![user1, user2],
            vec![sub1, sub2],
            vec![UserId::new(1)],
            false,
            false,
        );

        let summary = cmd.execute().await.unwrap();

        assert_eq!(
            summary.expired, 1,
            "второй юзер должен обработаться, несмотря на провал первого"
        );
        assert_eq!(summary.failed, 1);
        assert_eq!(vpn_calls.lock().unwrap().len(), 2, "попытка была для обоих");
        let updated_subs = updated.lock().unwrap();
        assert_eq!(updated_subs.len(), 1);
        assert_eq!(updated_subs[0].user_id(), UserId::new(2));
        assert_eq!(*notify_calls.lock().unwrap(), vec![UserId::new(2)]);
    }
}
