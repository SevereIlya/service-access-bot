use super::entity::Subscription;
use crate::domain::error::DomainResult;
use crate::domain::user::UserId;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

pub type DynSubscriptionRepository = Arc<dyn SubscriptionRepository + Send + Sync>;

/// Порт для хранения и поиска подписок.
#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    /// Сохраняет новую подписку.
    ///
    /// Возвращает [`SubscriptionError::AlreadyHasActive`], если у пользователя уже есть активная подписка.
    async fn create(&self, subscription: &Subscription) -> DomainResult<()>;

    /// Обновляет сохранённую подписку.
    ///
    /// Возвращает [`SubscriptionError::EntityNotSaved`], если подписка не имеет идентификатора.
    async fn update(&self, subscription: &Subscription) -> DomainResult<()>;

    /// Возвращает активную подписку пользователя, если она существует.
    ///
    /// Подписка считается активной, если её статус равен `Active` и срок действия ещё не истёк.
    async fn find_active_by_user_id(
        &self,
        user_id: UserId,
    ) -> DomainResult<Option<Subscription>>;

    /// Возвращает подписки со статусом `Active`, срок действия которых уже истёк.
    async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>>;

    /// Возвращает активные подписки, срок действия которых попадает в переданный временной интервал.
    ///
    /// Границы интервала включаются в поиск.
    async fn find_expiring_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> DomainResult<Vec<Subscription>>;
}
