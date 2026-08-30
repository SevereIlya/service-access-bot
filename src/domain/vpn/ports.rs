use crate::domain::error::DomainResult;
use crate::domain::subscription::SubscriptionDevices;
use crate::domain::user::User;
use crate::domain::vpn::Node;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub type DynVpnAccessRevoker = Arc<dyn VpnAccessRevoker + Send + Sync>;
pub type DynVpnProvisioner = Arc<dyn VpnProvisioner + Send + Sync>;
pub type DynVpnConfigGenerator = Arc<dyn VpnConfigGenerator + Send + Sync>;

/// Порт для отзыва VPN-доступа пользователя на всех нодах.
#[async_trait]
pub trait VpnAccessRevoker: Send + Sync {
    /// Отзывает VPN-доступ пользователя на всех нодах.
    async fn revoke_all(&self, user: &User) -> DomainResult<()>;
}

/// Порт для предоставления пользователю VPN-доступа.
#[async_trait]
pub trait VpnProvisioner: Send + Sync {
    /// Настраивает VPN-доступ для пользователя.
    async fn provision_node(
        &self,
        node: &Node,
        user: &User,
        devices: SubscriptionDevices,
    ) -> DomainResult<()>;
}

pub trait VpnConfigGenerator: Send + Sync {
    fn generate(&self, nodes: &[&Node], user_uuid: Uuid) -> DomainResult<String>;
}
