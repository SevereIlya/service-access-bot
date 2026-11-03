use crate::domain::{
    DomainResult,
    referral::{Referral, ReferralId},
    user::UserId,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynReferralRepository = Arc<dyn ReferralRepository + Send + Sync>;

#[async_trait]
pub trait ReferralRepository: Send + Sync {
    /// Сохраняет реферала.
    async fn create(&self, referral: &Referral) -> DomainResult<ReferralId>;

    /// Найти того, кто пригласил этого пользователя. Если `None`, значит никто.
    async fn find_by_referred_id(&self, referred_id: UserId) -> DomainResult<Option<Referral>>;

    /// Возвращает количество рефералов, приглашённых указанным пользователем.
    async fn count_by_referrer(&self, referrer_id: UserId) -> DomainResult<i64>;
}
