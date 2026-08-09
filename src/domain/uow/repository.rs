use crate::domain::error::DomainResult;
use crate::domain::subscription::repository::DynSubscriptionRepository;
use crate::domain::user::repository::DynUserRepository;
use async_trait::async_trait;
use std::sync::Arc;

pub type BoxedUowContext = Box<dyn UowContext>;
pub type DynUnitOfWork = Arc<dyn UnitOfWork>;

#[async_trait]
pub trait UnitOfWork: Send + Sync {
    async fn begin(&self) -> DomainResult<BoxedUowContext>;
}
#[async_trait]
pub trait UowContext: Send + Sync {
    fn users(&self) -> DynUserRepository;
    fn subscriptions(&self) -> DynSubscriptionRepository;

    async fn commit(&mut self) -> DomainResult<()>;
    async fn rollback(&mut self) -> DomainResult<()>;
}