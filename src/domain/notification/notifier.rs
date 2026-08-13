use crate::domain::error::DomainResult;
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
}
