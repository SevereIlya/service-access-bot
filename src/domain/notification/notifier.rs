use crate::domain::DomainResult;
use crate::domain::billing::Money;
use crate::domain::device::{DeviceName, DeviceSetupCode};
use crate::domain::user::User;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

pub type DynNotifier = Arc<dyn Notifier + Send + Sync>;

/// Порт для отправки пользователю уведомлений о событиях, не связанных с его текущим диалогом.
#[async_trait]
pub trait Notifier: Send + Sync {
    /// Уведомляет пользователя об истечении подписки.
    async fn notify_subscription_expired(&self, user: &User) -> DomainResult<()>;

    /// Уведомляет пользователя о приближении окончания подписки.
    async fn notify_subscription_expiring(
        &self,
        user: &User,
        expires_at: DateTime<Utc>,
    ) -> DomainResult<()>;

    /// Уведомляет о зачислении пополнения баланса.
    async fn notify_balance_topped_up(
        &self,
        user: &User,
        amount: Money,
        new_balance: Money,
    ) -> DomainResult<()>;

    /// Уведомляет о новом устройстве, зарегистрированном по HWID.
    async fn notify_new_device_detected(
        &self,
        user: &User,
        device_name: &DeviceName,
    ) -> DomainResult<()>;

    /// Отправляет код для устройства в статусе `Pending` с инструкцией, куда его вписать.
    async fn notify_device_setup_code(
        &self,
        user: &User,
        device_name: &DeviceName,
        code: &DeviceSetupCode,
    ) -> DomainResult<()>;
}
