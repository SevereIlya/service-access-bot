use crate::domain::{
    DeviceError, DomainError, DomainResult,
    device::{Device, DeviceFingerprint, DeviceRepository},
    user::UserId,
};
use crate::exec_query;
use crate::infrastructure::{DeviceRow, SharedTransaction, SqlxExecutor};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

#[must_use]
pub struct SqlxDeviceRepository {
    executor: SqlxExecutor,
}

impl SqlxDeviceRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self {
            executor: SqlxExecutor::Pool(pool),
        }
    }

    pub const fn transaction(tx: SharedTransaction) -> Self {
        Self {
            executor: SqlxExecutor::Transaction(tx),
        }
    }
}

#[async_trait]
impl DeviceRepository for SqlxDeviceRepository {
    #[instrument(skip(self, device), fields(user_id = %device.user_id()))]
    async fn create(&self, device: &Device) -> DomainResult<()> {
        let signal = device.signal();

        let query = sqlx::query!(
            r#"
            INSERT INTO devices (
                user_id, fingerprint, name, status,
                hardware_id, os_name, os_version, device_model, user_agent,
                first_seen_at, last_seen_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            device.user_id().inner(),
            device.fingerprint().inner(),
            device.name().inner(),
            device.status().as_str(),
            signal.hardware_id(),
            signal.os_name(),
            signal.os_version(),
            signal.device_model(),
            signal.user_agent(),
            device.first_seen_at(),
            device.last_seen_at(),
        );

        // добавить 23505 потом
        exec_query!(self.executor, query, execute)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self, device), fields(user_id = %device.user_id()))]
    async fn update(&self, device: &Device) -> DomainResult<()> {
        let id = device.id().ok_or(DomainError::Device(DeviceError::EntityNotSaved))?;

        let query = sqlx::query!(
            r#"
            UPDATE devices
            SET status = $1,
                last_seen_at = $2
            WHERE id = $3
            "#,
            device.status().as_str(),
            device.last_seen_at(),
            id.inner(),
        );

        exec_query!(self.executor, query, execute)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self, fingerprint))]
    async fn find_by_user_and_fingerprint(
        &self,
        user_id: UserId,
        fingerprint: &DeviceFingerprint,
    ) -> DomainResult<Option<Device>> {
        let query = sqlx::query_as!(
            DeviceRow,
            r#"
            SELECT *
            FROM devices
            WHERE user_id = $1
              AND fingerprint = $2
            "#,
            user_id.inner(),
            fingerprint.inner(),
        );

        let row: Option<DeviceRow> = exec_query!(self.executor, query, fetch_optional)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        row.map(TryInto::try_into).transpose()
    }

    #[instrument(skip(self))]
    async fn find_all_by_user(&self, user_id: UserId) -> DomainResult<Vec<Device>> {
        let query = sqlx::query_as!(
            DeviceRow,
            r#"
            SELECT *
            FROM devices
            WHERE user_id = $1
            ORDER BY id
            "#,
            user_id.inner(),
        );

        let rows: Vec<DeviceRow> = exec_query!(self.executor, query, fetch_all)
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        billing::Money,
        device::{DeviceName, DeviceSetupCode, DeviceSignal, DeviceStatus},
        user::{ReferralCode, SubscriptionToken, TelegramId, User, UserRepository},
    };
    use crate::infrastructure::SqlxUserRepository;
    use chrono::{TimeDelta, Utc};
    use uuid::Uuid;

    // ==========================================
    // Хелперы
    // ==========================================

    async fn seed_user(pool: &PgPool, n: i64) -> UserId {
        let user = User::new(
            TelegramId::new(1000 + n),
            Uuid::new_v4(),
            Some(format!("user{n}")),
            format!("User {n}"),
            Money::new(15_000).unwrap(),
            ReferralCode::new(format!("REF{n}")),
            SubscriptionToken::new(format!("TOK{n}")),
        );
        SqlxUserRepository::new(pool.clone()).create(&user).await.unwrap()
    }

    fn repo(pool: &PgPool) -> SqlxDeviceRepository {
        SqlxDeviceRepository::new(pool.clone())
    }

    fn hwid_device(user_id: UserId, hwid: &str) -> Device {
        Device::new_active(
            user_id,
            DeviceFingerprint::from_hwid(hwid),
            DeviceName::restore(format!("Device {hwid}")),
            DeviceSignal::new(Some(hwid.to_owned()), None, None, None, None),
        )
    }

    fn pending_device(user_id: UserId, code: &DeviceSetupCode) -> Device {
        Device::new_pending(
            user_id,
            DeviceFingerprint::from_code(code),
            DeviceName::restore("Clash Verge 01".to_owned()),
            DeviceSignal::new(
                None,
                None,
                None,
                None,
                Some("clash-verge/v2.5.6".to_owned()),
            ),
        )
    }

    // ==========================================
    // create + read
    // ==========================================

    #[sqlx::test]
    async fn create_roundtrips_every_field(pool: PgPool) {
        let user_id = seed_user(&pool, 1).await;
        let repo = repo(&pool);
        let device = Device::new_active(
            user_id,
            DeviceFingerprint::from_hwid("hw-1"),
            DeviceName::restore("Pixel 8".to_owned()),
            DeviceSignal::new(
                Some("hw-1".to_owned()),
                Some("Android".to_owned()),
                Some("14".to_owned()),
                Some("Pixel 8".to_owned()),
                Some("Happ/1.0.5".to_owned()),
            ),
        );

        repo.create(&device).await.unwrap();

        let found = repo.find_all_by_user(user_id).await.unwrap();

        assert_eq!(found.len(), 1);

        let stored = &found[0];

        assert!(stored.id().is_some(), "id должна выдать БД");
        assert_eq!(stored.user_id(), user_id);
        assert_eq!(stored.fingerprint(), device.fingerprint());
        assert_eq!(stored.name(), device.name());
        assert_eq!(stored.signal(), device.signal());
        assert_eq!(stored.status(), DeviceStatus::Active);

        assert_eq!(
            stored.first_seen_at().timestamp_micros(),
            device.first_seen_at().timestamp_micros()
        );
        assert_eq!(
            stored.last_seen_at().timestamp_micros(),
            device.last_seen_at().timestamp_micros()
        );
    }

    #[sqlx::test]
    async fn absent_signal_fields_roundtrip_as_none(pool: PgPool) {
        let user_id = seed_user(&pool, 1).await;
        let repo = repo(&pool);
        let code = DeviceSetupCode::generate();

        repo.create(&pending_device(user_id, &code)).await.unwrap();

        let found = repo.find_all_by_user(user_id).await.unwrap();
        let stored = &found[0];

        assert_eq!(stored.status(), DeviceStatus::Pending);
        assert_eq!(stored.fingerprint().as_setup_code(), Some(code));
        assert_eq!(stored.signal().hardware_id(), None);
        assert_eq!(stored.signal().os_name(), None);
        assert_eq!(stored.signal().os_version(), None);
        assert_eq!(stored.signal().device_model(), None);
        assert_eq!(stored.signal().user_agent(), Some("clash-verge/v2.5.6"));
    }

    // ==========================================
    // update
    // ==========================================

    #[sqlx::test]
    async fn update_writes_status_and_last_seen_and_keeps_the_rest(pool: PgPool) {
        let user_id = seed_user(&pool, 1).await;
        let repo = repo(&pool);
        let code = DeviceSetupCode::generate();

        repo.create(&pending_device(user_id, &code)).await.unwrap();

        let mut stored = repo.find_all_by_user(user_id).await.unwrap().remove(0);
        let original = stored.clone();
        let later = Utc::now() + TimeDelta::try_hours(1).unwrap();
        stored.confirm().unwrap();
        stored.mark_seen(later);

        repo.update(&stored).await.unwrap();

        let reloaded = repo.find_all_by_user(user_id).await.unwrap().remove(0);
        assert_eq!(reloaded.id(), original.id());
        assert_eq!(reloaded.status(), DeviceStatus::Active);
        assert_eq!(
            reloaded.last_seen_at().timestamp_micros(),
            later.timestamp_micros()
        );
        assert_eq!(reloaded.fingerprint(), original.fingerprint());
        assert_eq!(reloaded.name(), original.name());
        assert_eq!(reloaded.signal(), original.signal());
        assert_eq!(reloaded.first_seen_at(), original.first_seen_at());
    }

    #[sqlx::test]
    async fn update_of_unsaved_device_returns_entity_not_saved(pool: PgPool) {
        let repo = repo(&pool);
        let unsaved = hwid_device(UserId::new(1), "hw-1"); // id = None

        let result = repo.update(&unsaved).await;

        assert!(matches!(
            result,
            Err(DomainError::Device(DeviceError::EntityNotSaved))
        ));
    }

    // ==========================================
    // find
    // ==========================================

    #[sqlx::test]
    async fn find_by_user_and_fingerprint_is_scoped_to_user(pool: PgPool) {
        let user_a = seed_user(&pool, 1).await;
        let user_b = seed_user(&pool, 2).await;
        let repo = repo(&pool);

        repo.create(&hwid_device(user_a, "hw-1")).await.unwrap();

        let fingerprint = DeviceFingerprint::from_hwid("hw-1");

        let own = repo.find_by_user_and_fingerprint(user_a, &fingerprint).await.unwrap();
        let foreign = repo.find_by_user_and_fingerprint(user_b, &fingerprint).await.unwrap();
        let unknown = repo
            .find_by_user_and_fingerprint(user_a, &DeviceFingerprint::from_hwid("nope"))
            .await
            .unwrap();

        assert_eq!(
            own.expect("своё устройство должно найтись").fingerprint(),
            &fingerprint
        );
        assert!(
            foreign.is_none(),
            "чужой пользователь не видит это устройство"
        );
        assert!(unknown.is_none());
    }

    #[sqlx::test]
    async fn find_all_by_user_is_scoped_and_ordered(pool: PgPool) {
        let user_a = seed_user(&pool, 1).await;
        let user_b = seed_user(&pool, 2).await;
        let user_c = seed_user(&pool, 3).await;
        let repo = repo(&pool);

        repo.create(&hwid_device(user_a, "hw-a")).await.unwrap();
        repo.create(&hwid_device(user_b, "hw-x")).await.unwrap();
        repo.create(&hwid_device(user_a, "hw-b")).await.unwrap();

        let devices = repo.find_all_by_user(user_a).await.unwrap();
        let fingerprints: Vec<&str> = devices.iter().map(|d| d.fingerprint().inner()).collect();

        assert_eq!(
            fingerprints,
            ["hwid:hw-a", "hwid:hw-b"],
            "только свои, в порядке создания"
        );
        assert!(repo.find_all_by_user(user_c).await.unwrap().is_empty());
    }

    // ==========================================
    // Ограничения и битые данные
    // ==========================================

    #[sqlx::test]
    async fn duplicate_fingerprint_is_rejected_per_user_but_allowed_across_users(pool: PgPool) {
        let user_a = seed_user(&pool, 1).await;
        let user_b = seed_user(&pool, 2).await;
        let repo = repo(&pool);

        repo.create(&hwid_device(user_a, "hw-1")).await.unwrap();

        let duplicate = repo.create(&hwid_device(user_a, "hw-1")).await;

        // Фиксируем текущее поведение: unique-violation пока не превращается в доменную ошибку.
        // Когда появится AlreadyRegistered, этот assert сломается и подскажет, что тест надо обновить.
        assert!(matches!(duplicate, Err(DomainError::SystemFailure(_))));
        repo.create(&hwid_device(user_b, "hw-1")).await.unwrap();
    }

    #[sqlx::test]
    async fn unknown_status_in_db_fails_mapping(pool: PgPool) {
        let user_id = seed_user(&pool, 1).await;
        sqlx::query(
            "INSERT INTO devices (user_id, fingerprint, name, status, first_seen_at, last_seen_at) \
             VALUES ($1, 'hwid:broken', 'Broken', 'frozen', NOW(), NOW())",
        )
        .bind(user_id.inner())
        .execute(&pool)
        .await
        .unwrap();

        let result = repo(&pool).find_all_by_user(user_id).await;

        assert!(matches!(
            result,
            Err(DomainError::Device(DeviceError::InvalidStatus(status))) if status == "frozen"
        ));
    }

    #[sqlx::test]
    async fn deleting_user_cascades_to_devices(pool: PgPool) {
        let user_id = seed_user(&pool, 1).await;
        let repo = repo(&pool);
        repo.create(&hwid_device(user_id, "hw-1")).await.unwrap();

        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id.inner())
            .execute(&pool)
            .await
            .unwrap();

        assert!(repo.find_all_by_user(user_id).await.unwrap().is_empty());
    }
}
