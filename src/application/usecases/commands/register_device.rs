use crate::application::{AppError, AppResult};
use crate::domain::{
    DomainError, UserError,
    device::{
        AppClient, Device, DeviceFingerprint, DeviceIdentity, DeviceName, DeviceSetupCode,
        DeviceSignal, DeviceStatus, DynDeviceRepository,
    },
    notification::DynNotifier,
    subscription::{DynSubscriptionRepository, SubscriptionDevices},
    user::{User, UserId},
};
use chrono::{DateTime, Utc};
use tracing::{error, info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceAccess {
    /// Проверка пройдена, устройство подтверждено или только что зарегистрировано, можно выдавать конфиг.
    Granted,

    /// У пользователя нет активной подписки
    NoActiveSubscription,

    /// Clash Verge создан `Pending`, ждём, пока пользователь впишет код в User-Agent
    AwaitingSetup { code: DeviceSetupCode },

    /// `ClashMi` без включённого X-HWID
    NeedsHwid,

    /// В User-Agent лежит код, который этому пользователю не выдавался
    NotRegistered,

    /// Устройство отозвано, заново его не регистрируем
    Revoked,

    /// Clash-совместимый клиент без поддержки (`ClashMeta`)
    UnsupportedClient,

    /// Клиент не подходит (браузер, неизвестное приложение, Happ/INCY без поддержки YAML)
    RejectedClient,

    /// Все слоты лимита заняты (`Pending` + `Active`).
    LimitExceeded { limit: i32, current: i32 },
}

struct Registry<'a> {
    user: &'a User,
    user_id: UserId,
    limit: SubscriptionDevices,
    devices: Vec<Device>,
    now: DateTime<Utc>,
}

impl Registry<'_> {
    fn occupied(&self) -> i32 {
        let count = self.devices.iter().filter(|d| d.status().counts_toward_limit()).count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    fn has_slot(&self) -> bool {
        self.occupied() < self.limit.inner()
    }

    fn limit_exceeded(&self) -> DeviceAccess {
        DeviceAccess::LimitExceeded {
            limit: self.limit.inner(),
            current: self.occupied(),
        }
    }

    fn same_app_count(&self, app: Option<AppClient>) -> usize {
        app.map_or(0, |app| {
            self.devices.iter().filter(|d| d.signal().app_client() == Some(app)).count()
        })
    }

    fn find_mut(&mut self, fingerprint: &DeviceFingerprint) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.fingerprint() == fingerprint)
    }

    fn pending_of(&self, app: AppClient) -> Option<&Device> {
        self.devices.iter().find(|d| d.is_pending_of(app))
    }
}

/// Проверяет устройство на каждом запросе подписки и решает, отдавать ли рабочий конфиг.
pub struct RegisterDeviceCommand {
    subscription_repo: DynSubscriptionRepository,
    device_repo: DynDeviceRepository,
    notifier: DynNotifier,
}

impl RegisterDeviceCommand {
    pub fn new(
        subscription_repo: DynSubscriptionRepository,
        device_repo: DynDeviceRepository,
        notifier: DynNotifier,
    ) -> Self {
        Self {
            subscription_repo,
            device_repo,
            notifier,
        }
    }

    pub async fn execute(&self, user: &User, signal: DeviceSignal) -> AppResult<DeviceAccess> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;

        let identity = signal.identity();
        if matches!(identity, DeviceIdentity::Unknown) {
            return Ok(DeviceAccess::RejectedClient);
        }

        let Some(subscription) = self.subscription_repo.find_active_by_user_id(user_id).await?
        else {
            return Ok(DeviceAccess::NoActiveSubscription);
        };

        let mut registry = Registry {
            user,
            user_id,
            limit: subscription.devices(),
            devices: self.device_repo.find_all_by_user(user_id).await?,
            now: Utc::now(),
        };
        
        match identity {
            DeviceIdentity::Hwid(hwid) => self.on_hwid(&mut registry, &hwid, signal).await,
            DeviceIdentity::IssuedCode(code) => self.on_issued_code(&mut registry, &code).await,
            DeviceIdentity::MalformedCode => Ok(DeviceAccess::NotRegistered),
            DeviceIdentity::Unconfigured(app) => self.on_unconfigured(&registry, app, signal).await,
            DeviceIdentity::Unknown => Ok(DeviceAccess::RejectedClient),
        }
    }

    async fn on_hwid(
        &self,
        registry: &mut Registry<'_>,
        hwid: &str,
        signal: DeviceSignal,
    ) -> AppResult<DeviceAccess> {
        let fingerprint = DeviceFingerprint::from_hwid(hwid);
        let now = registry.now;

        if let Some(device) = registry.find_mut(&fingerprint) {
            return match device.status() {
                DeviceStatus::Revoked => Ok(DeviceAccess::Revoked),
                DeviceStatus::Pending | DeviceStatus::Active => self.grant(device, now).await,
            };
        }

        if !registry.has_slot() {
            return Ok(registry.limit_exceeded());
        }

        let name =
            DeviceName::for_new_device(&signal, registry.same_app_count(signal.app_client()));
        let device = Device::new_active(registry.user_id, fingerprint, name, signal);
        self.device_repo.create(&device).await?;
        info!(user_id = %registry.user_id, device = %device.name(), "зарегистрировано устройство по hwid");

        self.notify_new_device(registry, &device).await;
        Ok(DeviceAccess::Granted)
    }

    async fn on_issued_code(
        &self,
        registry: &mut Registry<'_>,
        code: &DeviceSetupCode,
    ) -> AppResult<DeviceAccess> {
        let fingerprint = DeviceFingerprint::from_code(code);
        let now = registry.now;

        let Some(device) = registry.find_mut(&fingerprint) else {
            return Ok(DeviceAccess::NotRegistered);
        };

        match device.status() {
            DeviceStatus::Revoked => return Ok(DeviceAccess::Revoked),
            DeviceStatus::Pending => device.confirm()?, // Pending в Active
            DeviceStatus::Active => { /* ничего */ }
        }

        self.grant(device, now).await
    }

    async fn on_unconfigured(
        &self,
        registry: &Registry<'_>,
        app: AppClient,
        signal: DeviceSignal,
    ) -> AppResult<DeviceAccess> {
        match app {
            AppClient::ClashVerge => self.on_clash_verge(registry, signal).await,
            AppClient::ClashMi => Ok(DeviceAccess::NeedsHwid),
            AppClient::ClashMeta => Ok(DeviceAccess::UnsupportedClient),
            AppClient::Happ | AppClient::Incy => Ok(DeviceAccess::RejectedClient),
        }
    }

    async fn on_clash_verge(
        &self,
        registry: &Registry<'_>,
        signal: DeviceSignal,
    ) -> AppResult<DeviceAccess> {
        if let Some(pending) = registry.pending_of(AppClient::ClashVerge) {
            let Some(code) = pending.fingerprint().as_setup_code() else {
                error!(
                    user_id = %registry.user_id,
                    fingerprint = %pending.fingerprint(),
                    "pending-устройство Clash Verge без кода в фингерпринте"
                );
                return Err(AppError::Internal(
                    "Повреждённая pending-запись устройства".into(),
                ));
            };
            return Ok(DeviceAccess::AwaitingSetup { code });
        }

        if !registry.has_slot() {
            return Ok(registry.limit_exceeded());
        }

        let code = DeviceSetupCode::generate();
        let name = DeviceName::for_new_device(
            &signal,
            registry.same_app_count(Some(AppClient::ClashVerge)),
        );
        let device = Device::new_pending(
            registry.user_id,
            DeviceFingerprint::from_code(&code),
            name,
            signal,
        );
        self.device_repo.create(&device).await?;
        info!(user_id = %registry.user_id, device = %device.name(), "создано pending-устройство Clash Verge");

        self.notify_setup_code(registry, &device, &code).await;
        Ok(DeviceAccess::Granted)
    }

    async fn grant(&self, device: &mut Device, now: DateTime<Utc>) -> AppResult<DeviceAccess> {
        device.mark_seen(now);
        self.device_repo.update(device).await?;
        Ok(DeviceAccess::Granted)
    }

    async fn notify_new_device(&self, registry: &Registry<'_>, device: &Device) {
        if let Err(e) = self.notifier.notify_new_device_detected(registry.user, device.name()).await
        {
            warn!(user_id = %registry.user_id, error = %e, "не удалось отправить уведомление о новом устройстве");
        }
    }

    async fn notify_setup_code(
        &self,
        registry: &Registry<'_>,
        device: &Device,
        code: &DeviceSetupCode,
    ) {
        if let Err(e) =
            self.notifier.notify_device_setup_code(registry.user, device.name(), code).await
        {
            warn!(user_id = %registry.user_id, error = %e, "не удалось отправить код настройки устройства");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainResult,
        billing::Money,
        device::{DeviceId, DeviceRepository},
        notification::Notifier,
        subscription::{
            Subscription, SubscriptionPlan, SubscriptionRepository, SubscriptionStatus,
        },
        user::{ReferralCode, SubscriptionToken, TelegramId},
    };
    use async_trait::async_trait;
    use chrono::Days;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    const CODE: &str = "0123456789abcdef0123456789abcdef";
    const CLASH_VERGE_UA: &str = "clash-verge/v2.5.6";
    const CLASHMI_UA: &str = "ClashMeta/1.19.30; mihomo/1.19.30";
    const CLASHMETA_ANDROID_UA: &str = "ClashMetaForAndroid/2.8.9";
    const HAPP_UA: &str = "Happ/1.0.5";
    const INCY_UA: &str = "Incy/2.1.0";
    const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

    // ==========================================
    // Моки
    // ==========================================

    /// Общий журнал побочных эффектов.
    #[derive(Clone, Default)]
    struct Spy {
        created: Arc<Mutex<Vec<Device>>>,
        updated: Arc<Mutex<Vec<Device>>>,
        new_device_notices: Arc<Mutex<Vec<String>>>,
        setup_code_notices: Arc<Mutex<Vec<DeviceSetupCode>>>,
    }

    impl Spy {
        fn created(&self) -> Vec<Device> {
            self.created.lock().unwrap().clone()
        }
        fn updated(&self) -> Vec<Device> {
            self.updated.lock().unwrap().clone()
        }
        fn new_device_notices(&self) -> Vec<String> {
            self.new_device_notices.lock().unwrap().clone()
        }
        fn setup_code_notices(&self) -> Vec<DeviceSetupCode> {
            self.setup_code_notices.lock().unwrap().clone()
        }

        fn assert_untouched(&self) {
            assert!(self.created().is_empty(), "ничего не должно создаваться");
            assert!(self.updated().is_empty(), "ничего не должно обновляться");
            assert!(
                self.new_device_notices().is_empty(),
                "уведомления о новом устройстве быть не должно"
            );
            assert!(
                self.setup_code_notices().is_empty(),
                "уведомления с кодом быть не должно"
            );
        }
    }

    struct MockSubscriptionRepository {
        active: Option<Subscription>,
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
            Ok(self.active.clone())
        }
        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_lapsed_active() не используется в этом юзкейсе")
        }
        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_due_for_expiry_warning() не используется в этом юзкейсе")
        }
    }

    struct MockDeviceRepository {
        devices: Vec<Device>,
        spy: Spy,
    }

    #[async_trait]
    impl DeviceRepository for MockDeviceRepository {
        async fn create(&self, device: &Device) -> DomainResult<()> {
            self.spy.created.lock().unwrap().push(device.clone());
            Ok(())
        }
        async fn update(&self, device: &Device) -> DomainResult<()> {
            self.spy.updated.lock().unwrap().push(device.clone());
            Ok(())
        }
        async fn find_by_user_and_fingerprint(
            &self,
            _user_id: UserId,
            _fingerprint: &DeviceFingerprint,
        ) -> DomainResult<Option<Device>> {
            unreachable!("юзкейс читает реестр одним снимком через find_all_by_user()")
        }
        async fn find_all_by_user(&self, _user_id: UserId) -> DomainResult<Vec<Device>> {
            Ok(self.devices.clone())
        }
    }

    struct MockNotifier {
        fail: bool,
        spy: Spy,
    }

    impl MockNotifier {
        fn outcome(&self) -> DomainResult<()> {
            if self.fail {
                return Err(DomainError::SystemFailure("notify failed (test)".into()));
            }
            Ok(())
        }
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
            device_name: &DeviceName,
        ) -> DomainResult<()> {
            self.spy.new_device_notices.lock().unwrap().push(device_name.inner().to_owned());
            self.outcome()
        }
        async fn notify_device_setup_code(
            &self,
            _user: &User,
            _device_name: &DeviceName,
            code: &DeviceSetupCode,
        ) -> DomainResult<()> {
            self.spy.setup_code_notices.lock().unwrap().push(code.clone());
            self.outcome()
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
            Money::new(15_000).unwrap(),
            ReferralCode::new(format!("REF{id}")),
            SubscriptionToken::new(format!("TOK{id}")),
        );
        user.assign_id(UserId::new(id));
        user
    }

    fn make_active_subscription(user_id: i64, devices: i32) -> Subscription {
        let now = Utc::now();
        Subscription::new(
            UserId::new(user_id),
            SubscriptionPlan::Month1,
            now,
            now + Days::new(30),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(devices).unwrap(),
        )
    }

    fn make_signal(
        hwid: Option<&str>,
        model: Option<&str>,
        user_agent: Option<&str>,
    ) -> DeviceSignal {
        DeviceSignal::new(
            hwid.map(str::to_owned),
            None,
            None,
            model.map(str::to_owned),
            user_agent.map(str::to_owned),
        )
    }

    fn code_fingerprint() -> DeviceFingerprint {
        DeviceFingerprint::from_code(&DeviceSetupCode::parse(CODE).unwrap())
    }

    /// Устройство, уже лежащее в БД
    fn stored_device(
        id: i64,
        status: DeviceStatus,
        fingerprint: DeviceFingerprint,
        user_agent: &str,
    ) -> Device {
        let long_ago = Utc::now() - Days::new(1);
        Device::restore_from_db(
            DeviceId::new(id),
            UserId::new(1),
            fingerprint,
            DeviceName::restore(format!("Device {id}")),
            make_signal(None, None, Some(user_agent)),
            status,
            long_ago,
            long_ago,
        )
    }

    /// `limit = None` означает, что активной подписки нет
    fn setup(
        limit: Option<i32>,
        devices: Vec<Device>,
        fail_notify: bool,
    ) -> (RegisterDeviceCommand, Spy) {
        let spy = Spy::default();
        let cmd = RegisterDeviceCommand::new(
            Arc::new(MockSubscriptionRepository {
                active: limit.map(|n| make_active_subscription(1, n)),
            }),
            Arc::new(MockDeviceRepository {
                devices,
                spy: spy.clone(),
            }),
            Arc::new(MockNotifier {
                fail: fail_notify,
                spy: spy.clone(),
            }),
        );
        (cmd, spy)
    }

    // ==========================================
    // Общие проверки
    // ==========================================

    #[tokio::test]
    async fn user_without_id_returns_entity_not_saved() {
        let user = User::new(
            TelegramId::new(1),
            Uuid::new_v4(),
            None,
            "Nobody".to_owned(),
            Money::new(15_000).unwrap(),
            ReferralCode::new("REF1".to_owned()),
            SubscriptionToken::new("TOK1".to_owned()),
        );
        let (cmd, spy) = setup(Some(3), vec![], false);

        let result = cmd.execute(&user, make_signal(Some("hw-1"), None, None)).await;

        assert!(matches!(
            result,
            Err(AppError::Domain(DomainError::User(
                UserError::EntityNotSaved
            )))
        ));
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn no_active_subscription_skips_device_check() {
        let (cmd, spy) = setup(None, vec![], false);

        let access =
            cmd.execute(&make_user(1), make_signal(Some("hw-1"), None, Some(CLASHMI_UA))).await.unwrap();

        assert_eq!(access, DeviceAccess::NoActiveSubscription);
        spy.assert_untouched();
    }

    // ==========================================
    // Ветка Hwid
    // ==========================================

    #[tokio::test]
    async fn hwid_new_device_is_registered_active_and_announced() {
        let (cmd, spy) = setup(Some(3), vec![], false);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-1"), Some("Pixel 8"), Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        let created = spy.created();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].user_id(), UserId::new(1));
        assert_eq!(
            created[0].fingerprint(),
            &DeviceFingerprint::from_hwid("hw-1")
        );
        assert_eq!(created[0].status(), DeviceStatus::Active);
        assert_eq!(created[0].name().inner(), "Pixel 8");
        assert_eq!(spy.new_device_notices(), vec!["Pixel 8".to_owned()]);
        assert!(spy.setup_code_notices().is_empty());
    }

    #[tokio::test]
    async fn hwid_known_device_only_marks_seen() {
        let known = stored_device(
            1,
            DeviceStatus::Active,
            DeviceFingerprint::from_hwid("hw-1"),
            CLASHMI_UA,
        );
        let before = known.last_seen_at();
        let (cmd, spy) = setup(Some(3), vec![known], false);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-1"), None, Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        assert!(
            spy.created().is_empty(),
            "известное устройство не создаём заново"
        );
        assert!(
            spy.new_device_notices().is_empty(),
            "о знакомом устройстве не уведомляем"
        );
        let updated = spy.updated();
        assert_eq!(updated.len(), 1);
        assert!(
            updated[0].last_seen_at() > before,
            "last_seen_at должен обновиться"
        );
        assert_eq!(updated[0].status(), DeviceStatus::Active);
    }

    #[tokio::test]
    async fn hwid_revoked_device_is_denied_and_not_registered_again() {
        let revoked = stored_device(
            1,
            DeviceStatus::Revoked,
            DeviceFingerprint::from_hwid("hw-1"),
            CLASHMI_UA,
        );
        let (cmd, spy) = setup(Some(3), vec![revoked], false);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-1"), None, Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Revoked);
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn hwid_new_device_over_limit_is_denied() {
        let devices = vec![
            stored_device(
                1,
                DeviceStatus::Active,
                DeviceFingerprint::from_hwid("hw-a"),
                CLASHMI_UA,
            ),
            stored_device(2, DeviceStatus::Pending, code_fingerprint(), CLASH_VERGE_UA),
        ];
        let (cmd, spy) = setup(Some(2), devices, false);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-new"), None, Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(
            access,
            DeviceAccess::LimitExceeded {
                limit: 2,
                current: 2
            }
        );
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn revoked_devices_do_not_occupy_slots() {
        let revoked = stored_device(
            1,
            DeviceStatus::Revoked,
            DeviceFingerprint::from_hwid("hw-old"),
            CLASHMI_UA,
        );
        let (cmd, spy) = setup(Some(1), vec![revoked], false);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-new"), None, Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        assert_eq!(spy.created().len(), 1);
    }

    // ==========================================
    // Ветка IssuedCode
    // ==========================================

    #[tokio::test]
    async fn issued_code_unknown_to_user_is_not_registered() {
        let (cmd, spy) = setup(Some(3), vec![], false);

        let access = cmd.execute(&make_user(1), make_signal(None, None, Some(CODE))).await.unwrap();

        assert_eq!(access, DeviceAccess::NotRegistered);
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn issued_code_confirms_pending_device() {
        let pending = stored_device(1, DeviceStatus::Pending, code_fingerprint(), CLASH_VERGE_UA);
        let before = pending.last_seen_at();
        let (cmd, spy) = setup(Some(3), vec![pending], false);

        let access = cmd.execute(&make_user(1), make_signal(None, None, Some(CODE))).await.unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        assert!(spy.created().is_empty());
        let updated = spy.updated();
        assert_eq!(updated.len(), 1);
        assert_eq!(
            updated[0].status(),
            DeviceStatus::Active,
            "pending должен стать Active"
        );
        assert!(updated[0].last_seen_at() > before);
    }

    #[tokio::test]
    async fn issued_code_of_active_device_only_marks_seen() {
        let active = stored_device(1, DeviceStatus::Active, code_fingerprint(), CLASH_VERGE_UA);
        let (cmd, spy) = setup(Some(3), vec![active], false);

        let access = cmd.execute(&make_user(1), make_signal(None, None, Some(CODE))).await.unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        let updated = spy.updated();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].status(), DeviceStatus::Active);
    }

    #[tokio::test]
    async fn issued_code_of_revoked_device_is_denied() {
        let revoked = stored_device(1, DeviceStatus::Revoked, code_fingerprint(), CLASH_VERGE_UA);
        let (cmd, spy) = setup(Some(3), vec![revoked], false);

        let access = cmd.execute(&make_user(1), make_signal(None, None, Some(CODE))).await.unwrap();

        assert_eq!(access, DeviceAccess::Revoked);
        spy.assert_untouched();
    }

    // ==========================================
    // Ветка Unconfigured
    // ==========================================

    #[tokio::test]
    async fn clash_verge_first_request_creates_pending_and_grants_once() {
        let (cmd, spy) = setup(Some(3), vec![], false);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASH_VERGE_UA)))
            .await
            .unwrap();

        assert_eq!(
            access,
            DeviceAccess::Granted,
            "первый запрос получает рабочий конфиг"
        );
        let created = spy.created();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].status(), DeviceStatus::Pending);
        assert_eq!(created[0].name().inner(), "Clash Verge 01");
        let code = created[0]
            .fingerprint()
            .as_setup_code()
            .expect("fingerprint должен быть code:..");
        assert_eq!(spy.setup_code_notices(), vec![code]);
        assert!(spy.new_device_notices().is_empty());
    }

    #[tokio::test]
    async fn clash_verge_with_pending_awaits_setup_idempotently() {
        let pending = stored_device(1, DeviceStatus::Pending, code_fingerprint(), CLASH_VERGE_UA);
        // Лимит 1 и он занят самим pending: ответ всё равно AwaitingSetup, а не LimitExceeded.
        let (cmd, spy) = setup(Some(1), vec![pending], false);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASH_VERGE_UA)))
            .await
            .unwrap();

        assert_eq!(
            access,
            DeviceAccess::AwaitingSetup {
                code: DeviceSetupCode::parse(CODE).unwrap()
            }
        );
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn clash_verge_over_limit_is_denied() {
        let active = stored_device(
            1,
            DeviceStatus::Active,
            DeviceFingerprint::from_hwid("hw-a"),
            HAPP_UA,
        );
        let (cmd, spy) = setup(Some(1), vec![active], false);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASH_VERGE_UA)))
            .await
            .unwrap();

        assert_eq!(
            access,
            DeviceAccess::LimitExceeded {
                limit: 1,
                current: 1
            }
        );
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn clash_verge_numbering_counts_revoked_devices() {
        let revoked = stored_device(
            1,
            DeviceStatus::Revoked,
            DeviceFingerprint::from_hwid("hw-old"),
            CLASH_VERGE_UA,
        );
        let (cmd, spy) = setup(Some(3), vec![revoked], false);

        cmd.execute(&make_user(1), make_signal(None, None, Some(CLASH_VERGE_UA)))
            .await
            .unwrap();

        let created = spy.created();
        assert_eq!(
            created[0].name().inner(),
            "Clash Verge 02",
            "номер не должен переезжать"
        );
    }

    #[tokio::test]
    async fn clashmi_without_hwid_needs_hwid_and_gets_no_slot() {
        let (cmd, spy) = setup(Some(3), vec![], false);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASHMI_UA)))
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::NeedsHwid);
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn clash_meta_gets_the_unsupported_notice() {
        let (cmd, spy) = setup(Some(3), vec![], false);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASHMETA_ANDROID_UA)))
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::UnsupportedClient);
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn clients_that_cannot_read_our_yaml_are_rejected_and_never_registered() {
        let cases = [
            make_signal(Some("hw-1"), Some("SM-G965F"), Some(HAPP_UA)),
            make_signal(Some("hw-1"), Some("samsung SM-G965F"), Some(INCY_UA)),
            make_signal(None, None, Some(HAPP_UA)),
            make_signal(None, None, Some(BROWSER_UA)),
            make_signal(None, None, Some("curl/8.0")),
            make_signal(Some("hw-1"), None, None),
            make_signal(None, None, None),
        ];

        for (i, signal) in cases.into_iter().enumerate() {
            let (cmd, spy) = setup(Some(3), vec![], false);

            let access = cmd.execute(&make_user(1), signal).await.unwrap();

            assert_eq!(access, DeviceAccess::RejectedClient, "случай #{i}");
            spy.assert_untouched();
        }
    }

    #[tokio::test]
    async fn rejected_client_learns_nothing_about_the_subscription() {
        let (cmd, spy) = setup(None, vec![], false); // подписки нет

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(BROWSER_UA)))
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::RejectedClient, "а не NoActiveSubscription");
        spy.assert_untouched();
    }

    #[tokio::test]
    async fn malformed_code_is_reported_as_not_registered() {
        let too_long = format!("{CODE}a");

        for ua in [&CODE[..31], too_long.as_str()] {
            let (cmd, spy) = setup(Some(3), vec![], false);

            let access =
                cmd.execute(&make_user(1), make_signal(None, None, Some(ua))).await.unwrap();

            assert_eq!(access, DeviceAccess::NotRegistered, "UA: {ua}");
            spy.assert_untouched();
        }
    }

    // ==========================================
    // Уведомления
    // ==========================================

    #[tokio::test]
    async fn notifier_failure_does_not_break_hwid_registration() {
        let (cmd, spy) = setup(Some(3), vec![], true);

        let access = cmd
            .execute(
                &make_user(1),
                make_signal(Some("hw-1"), Some("Pixel 8"), Some(CLASHMI_UA)),
            )
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        assert_eq!(
            spy.created().len(),
            1,
            "устройство сохранено, несмотря на сбой Telegram"
        );
        assert_eq!(spy.new_device_notices().len(), 1, "попытка уведомить была");
    }

    #[tokio::test]
    async fn notifier_failure_does_not_break_clash_verge_registration() {
        let (cmd, spy) = setup(Some(3), vec![], true);

        let access = cmd
            .execute(&make_user(1), make_signal(None, None, Some(CLASH_VERGE_UA)))
            .await
            .unwrap();

        assert_eq!(access, DeviceAccess::Granted);
        assert_eq!(spy.created().len(), 1);
        assert_eq!(spy.setup_code_notices().len(), 1, "попытка уведомить была");
    }
}
