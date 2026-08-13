use crate::domain::error::DomainResult;
use crate::domain::user::{User, UserId};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynVpnAccessRevoker = Arc<dyn VpnAccessRevoker + Send + Sync>;
pub type DynVpnProvisioner = Arc<dyn VpnProvisioner + Send + Sync>;

/// Порт для отзыва VPN-доступа пользователя на всех нодах.
#[async_trait]
pub trait VpnAccessRevoker: Send + Sync {

    /// Отзывает VPN-доступ пользователя на всех нодах.
    async fn revoke_all(&self, user_id: UserId) -> DomainResult<()>;
}

/// Порт для предоставления пользователю VPN-доступа.
#[async_trait]
pub trait VpnProvisioner: Send + Sync {

    /// Настраивает VPN-доступ для пользователя.
    async fn provision(&self, user: &User) -> DomainResult<()>;
}
