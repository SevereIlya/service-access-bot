use crate::domain::device::{
    AppClient, DeviceFingerprint, DeviceId, DeviceName, DeviceSignal, DeviceStatus,
};
use crate::domain::user::UserId;
use crate::domain::{DeviceError, DomainResult};
use chrono::{DateTime, Utc};

/// Устройство пользователя в реестре.
///
/// При создании фиксируются отпечаток устройства (`fingerprint`), его имя (`name`) и исходный сигнал (`signal`).
/// Эти данные не изменяются в течение жизни записи.
///
/// Изменяемыми состояниями устройства являются только `status` и `last_seen_at`, изменяемые соответствующими методами.
#[derive(Debug, Clone)]
pub struct Device {
    id: Option<DeviceId>,
    user_id: UserId,
    fingerprint: DeviceFingerprint,
    name: DeviceName,
    signal: DeviceSignal,
    status: DeviceStatus,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl Device {
    /// Создаёт новое устройство в состоянии `Active`.
    ///
    /// Используется, когда клиент сразу предоставляет надёжный идентификатор, например HWID.
    #[must_use]
    pub fn new_active(
        user_id: UserId,
        fingerprint: DeviceFingerprint,
        name: DeviceName,
        signal: DeviceSignal,
    ) -> Self {
        Self::create(user_id, fingerprint, name, signal, DeviceStatus::Active)
    }

    /// Создаёт новое устройство в состоянии `Pending`.
    ///
    /// Используется, когда клиент ещё не подтвержден и ожидается подтверждение через выданный код настройки.
    #[must_use]
    pub fn new_pending(
        user_id: UserId,
        fingerprint: DeviceFingerprint,
        name: DeviceName,
        signal: DeviceSignal,
    ) -> Self {
        Self::create(user_id, fingerprint, name, signal, DeviceStatus::Pending)
    }

    fn create(
        user_id: UserId,
        fingerprint: DeviceFingerprint,
        name: DeviceName,
        signal: DeviceSignal,
        status: DeviceStatus,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            user_id,
            fingerprint,
            name,
            signal,
            status,
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    /// Восстанавливает устройство из БД.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn restore_from_db(
        id: DeviceId,
        user_id: UserId,
        fingerprint: DeviceFingerprint,
        name: DeviceName,
        signal: DeviceSignal,
        status: DeviceStatus,
        first_seen_at: DateTime<Utc>,
        last_seen_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            user_id,
            fingerprint,
            name,
            signal,
            status,
            first_seen_at,
            last_seen_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Option<DeviceId> {
        self.id
    }

    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &DeviceFingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub const fn name(&self) -> &DeviceName {
        &self.name
    }

    #[must_use]
    pub const fn signal(&self) -> &DeviceSignal {
        &self.signal
    }

    #[must_use]
    pub const fn status(&self) -> DeviceStatus {
        self.status
    }

    #[must_use]
    pub const fn first_seen_at(&self) -> DateTime<Utc> {
        self.first_seen_at
    }

    #[must_use]
    pub const fn last_seen_at(&self) -> DateTime<Utc> {
        self.last_seen_at
    }

    /// Проверяет, ожидает ли устройство подтверждения для указанного клиента.
    ///
    /// Возвращает `true` только для устройства в состоянии `Pending`, исходный сигнал которого относится к указанному [`AppClient`].
    ///
    /// Нужен, чтобы повторный запрос Clash Verge с дефолтным UA нашёл свою pending-запись, а не создал новую.
    #[must_use]
    pub fn is_pending_of(&self, app: AppClient) -> bool {
        self.status == DeviceStatus::Pending && self.signal.app_client() == Some(app)
    }

    /// Обновляет время последнего обращения устройства.
    pub const fn mark_seen(&mut self, now: DateTime<Utc>) {
        self.last_seen_at = now;
    }

    /// Подтверждает устройство, переводя его из `Pending` в `Active`.
    ///
    /// Для устройства в любом другом состоянии возвращает [`DeviceError::InvalidStatusTransition`], не изменяя его состояние.
    pub fn confirm(&mut self) -> DomainResult<()> {
        match self.status {
            DeviceStatus::Pending => {
                self.status = DeviceStatus::Active;
                Ok(())
            }
            from => Err(DeviceError::InvalidStatusTransition {
                from,
                to: DeviceStatus::Active,
            }
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::DomainError;
    use chrono::TimeDelta;

    const CLASH_VERGE_UA: &str = "clash-verge/v2.5.6";

    fn signal() -> DeviceSignal {
        DeviceSignal::new(
            Some("hw-1".to_owned()),
            Some("Android".to_owned()),
            Some("14".to_owned()),
            Some("Pixel 8".to_owned()),
            Some(CLASH_VERGE_UA.to_owned()),
        )
    }

    fn name() -> DeviceName {
        DeviceName::restore("Test device".to_owned())
    }

    /// Устройство из БД в нужном статусе.
    /// Пока не предусмотрено нужного метода, это единственный способ получить `Revoked` в тесте.
    fn stored(status: DeviceStatus) -> Device {
        let now = Utc::now();
        Device::restore_from_db(
            DeviceId::new(1),
            UserId::new(7),
            DeviceFingerprint::from_hwid("hw-1"),
            name(),
            signal(),
            status,
            now,
            now,
        )
    }

    #[test]
    fn new_active_device_is_unsaved_and_active() {
        let s = signal();
        let n = name();
        let fp = DeviceFingerprint::from_hwid("hw-1");

        let device = Device::new_active(UserId::new(7), fp.clone(), n.clone(), s.clone());

        assert!(
            device.id().is_none(),
            "У нового устройства не должно быть ID"
        );
        assert_eq!(device.user_id().inner(), 7);
        assert_eq!(device.fingerprint(), &fp, "Фингерпринт должен совпадать");
        assert_eq!(device.name(), &n, "Имя должно совпадать");
        assert_eq!(
            device.signal(),
            &s,
            "Сигнал должен сохраниться без изменений"
        );
        assert_eq!(device.status(), DeviceStatus::Active);
        assert_eq!(
            device.first_seen_at(),
            device.last_seen_at(),
            "Даты первого и последнего визита должны совпадать при создании"
        );
    }

    #[test]
    fn new_pending_device_waits_for_confirmation() {
        let device = Device::new_pending(
            UserId::new(7),
            DeviceFingerprint::from_hwid("hw-1"),
            name(),
            signal(),
        );

        assert!(device.id().is_none());
        assert_eq!(
            device.status(),
            DeviceStatus::Pending,
            "Статус должен быть Pending"
        );
        assert_eq!(device.first_seen_at(), device.last_seen_at());
    }

    #[test]
    fn restore_from_db_keeps_stored_values() {
        let now = Utc::now();
        let later = now + TimeDelta::try_seconds(10).unwrap();

        let s = signal();
        let n = name();

        let device = Device::restore_from_db(
            DeviceId::new(42),
            UserId::new(7),
            DeviceFingerprint::from_hwid("hw-1"),
            n.clone(),
            s.clone(),
            DeviceStatus::Revoked,
            now,
            later,
        );

        assert_eq!(device.id(), Some(DeviceId::new(42)));
        assert_eq!(device.user_id(), UserId::new(7));
        assert_eq!(device.fingerprint(), &DeviceFingerprint::from_hwid("hw-1"));
        assert_eq!(device.name(), &n);
        assert_eq!(device.signal(), &s);
        assert_eq!(device.status(), DeviceStatus::Revoked);
        assert_eq!(device.first_seen_at(), now);
        assert_eq!(device.last_seen_at(), later);
    }

    #[test]
    fn mark_seen_changes_only_last_seen_at() {
        let mut device = stored(DeviceStatus::Active);
        let first_seen = device.first_seen_at();
        let later = first_seen + TimeDelta::try_seconds(10).unwrap();

        device.mark_seen(later);

        assert_eq!(
            device.last_seen_at(),
            later,
            "last_seen_at должен обновиться"
        );
        assert_eq!(
            device.first_seen_at(),
            first_seen,
            "first_seen_at НЕ должен измениться"
        );
        assert_eq!(
            device.status(),
            DeviceStatus::Active,
            "Статус НЕ должен измениться"
        );
    }

    #[test]
    fn confirm_activates_pending_device() {
        let mut device = stored(DeviceStatus::Pending);

        let result = device.confirm();

        assert!(
            result.is_ok(),
            "Переход из Pending в Active должен быть успешным"
        );
        assert_eq!(device.status(), DeviceStatus::Active);
    }

    #[test]
    fn confirm_rejects_anything_but_pending() {
        for status in [DeviceStatus::Active, DeviceStatus::Revoked] {
            let mut device = stored(status);
            let original_last_seen = device.last_seen_at();

            let err = device.confirm().unwrap_err();

            assert!(matches!(
                err,
                DomainError::Device(DeviceError::InvalidStatusTransition {
                    from,
                    to: DeviceStatus::Active
                }) if from == status
            ));

            assert_eq!(
                device.status(),
                status,
                "При ошибке статус не должен меняться"
            );
            assert_eq!(
                device.last_seen_at(),
                original_last_seen,
                "При ошибке дата не должна меняться"
            );
        }
    }

    #[test]
    fn is_pending_of_checks_both_status_and_app() {
        let pending = stored(DeviceStatus::Pending);
        let active = stored(DeviceStatus::Active);

        // Правильное приложение + правильный статус - true
        assert!(
            pending.is_pending_of(AppClient::ClashVerge),
            "Pending устройство с ClashVerge возвращает true"
        );

        // Неправильное приложение - false
        assert!(
            !pending.is_pending_of(AppClient::ClashMi),
            "ClashMi возвращает false"
        );
        assert!(
            !pending.is_pending_of(AppClient::Happ),
            "Happ возвращает false"
        );
        assert!(
            !pending.is_pending_of(AppClient::Incy),
            "Incy возвращает false"
        );

        // Неправильный статус - false
        assert!(
            !active.is_pending_of(AppClient::ClashVerge),
            "Active устройство возвращает false даже при совпадении приложения"
        );
    }
}
