use super::entity::Subscription;
use crate::domain::error::DomainResult;
use crate::domain::user::UserId;
use async_trait::async_trait;
use std::sync::Arc;

pub type DynSubscriptionRepository = Arc<dyn SubscriptionRepository + Send + Sync>;

#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    async fn create(&self, subscription: &Subscription) -> DomainResult<()>;
    async fn find_active_by_user_id(
        &self,
        user_id: UserId,
    ) -> DomainResult<Option<Subscription>>;
}
