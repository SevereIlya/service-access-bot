use crate::domain::error::DomainResult;
use crate::domain::user::UserId;
use crate::domain::vpn::{Node, NodeId, VpnConnection};
use async_trait::async_trait;
use std::sync::Arc;

pub type DynNodeRepository = Arc<dyn NodeRepository + Send + Sync>;
pub type DynVpnConnectionRepository = Arc<dyn VpnConnectionRepository + Send + Sync>;

#[async_trait]
pub trait NodeRepository: Send + Sync {
    /// Возвращает все активные VPN-ноды.
    async fn find_active_nodes(&self) -> DomainResult<Vec<Node>>;
}

#[async_trait]
pub trait VpnConnectionRepository: Send + Sync {
    /// Возвращает VPN-соединение пользователя с указанной нодой, если оно существует.
    async fn find_by_user_and_node(
        &self,
        user_id: UserId,
        node_id: NodeId,
    ) -> DomainResult<Option<VpnConnection>>;

    /// Сохраняет VPN-соединение пользователя с нодой.
    ///
    /// Если соединение уже существует, обновляет его состояние синхронизации.
    async fn upsert(&self, connection: &VpnConnection) -> DomainResult<()>;
}
