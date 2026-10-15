use crate::domain::{
    DomainResult,
    device::{Device, DeviceFingerprint},
    user::UserId,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynDeviceRepository = Arc<dyn DeviceRepository + Send + Sync>;

/// Репозиторий для хранения и поиска устройств пользователей.
#[async_trait]
pub trait DeviceRepository: Send + Sync {
    /// Сохраняет новое устройство.
    async fn create(&self, device: &Device) -> DomainResult<()>;

    /// Обновляет существующее устройство.
    ///
    /// Пишет только изменяемые поля `status` и `last_seen_at`.
    /// `fingerprint`, `name` и `signal` неизменяемые и в `UPDATE` не участвуют.
    ///
    /// Возвращает `DeviceError::EntityNotSaved`, если устройство не сохранено.
    async fn update(&self, device: &Device) -> DomainResult<()>;

    /// Возвращает устройство пользователя по фингерпринту в любом статусе.
    async fn find_by_user_and_fingerprint(
        &self,
        user_id: UserId,
        fingerprint: &DeviceFingerprint,
    ) -> DomainResult<Option<Device>>;

    /// Возвращает все устройства пользователя в любом статусе.
    async fn find_all_by_user(&self, user_id: UserId) -> DomainResult<Vec<Device>>;
}
