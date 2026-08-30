use crate::domain::vpn::{NodeId, NodeIpAddress, VpnConnectionId};
use chrono::{DateTime, Utc};
use crate::domain::user::UserId;

/// VPN-нода, доступная для подключения пользователей.
#[derive(Debug, Clone)]
pub struct Node {
    id: Option<NodeId>,
    name: String,
    ip_address: NodeIpAddress,
    is_active: bool,
    created_at: DateTime<Utc>,
}

impl Node {
    /// Создаёт новую активную VPN-ноду.
    ///
    /// Идентификатор ноды назначается после сохранения в базе данных.
    #[must_use]
    pub fn new(name: String, ip_address: NodeIpAddress) -> Self {
        Self {
            id: None,
            name,
            ip_address,
            is_active: true,
            created_at: Utc::now(),
        }
    }

    /// Восстанавливает VPN-ноду из сохранённых в базе данных данных.
    #[must_use]
    pub const fn restore_from_db(
        id: NodeId,
        name: String,
        ip_address: NodeIpAddress,
        is_active: bool,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            name,
            ip_address,
            is_active,
            created_at,
        }
    }

    /// Возвращает идентификатор VPN-ноды, если он был назначен.
    #[must_use]
    pub const fn id(&self) -> Option<NodeId> {
        self.id
    }

    /// Возвращает имя VPN-ноды.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Возвращает IP-адрес VPN-ноды.
    #[must_use]
    pub const fn ip_address(&self) -> NodeIpAddress {
        self.ip_address
    }

    /// Проверяет, активна ли VPN-нода.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    /// Возвращает время создания VPN-ноды.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Активирует VPN-ноду.
    pub const fn activate(&mut self) {
        self.is_active = true;
    }

    /// Деактивирует VPN-ноду.
    pub const fn deactivate(&mut self) {
        self.is_active = false;
    }
}

// ============================================================================================== //

/// VPN-соединение пользователя с нодой.
#[derive(Debug, Clone)]
pub struct VpnConnection {
    id: Option<VpnConnectionId>,
    user_id: UserId,
    node_id: NodeId,
    is_synced: bool,
    created_at: DateTime<Utc>,
}

impl VpnConnection {
    /// Создаёт новое VPN-соединение пользователя с нодой.
    ///
    /// Новое соединение считается несинхронизированным с VPN-нодой.
    /// Идентификатор соединения назначается после сохранения в базе данных.
    #[must_use]
    pub fn new(user_id: UserId, node_id: NodeId) -> Self {
        Self {
            id: None,
            user_id,
            node_id,
            is_synced: false,
            created_at: Utc::now(),
        }
    }

    /// Восстанавливает VPN-соединение из сохранённых в базе данных данных.
    #[must_use]
    pub const fn restore_from_db(
        id: VpnConnectionId,
        user_id: UserId,
        node_id: NodeId,
        is_synced: bool,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            user_id,
            node_id,
            is_synced,
            created_at,
        }
    }

    /// Возвращает идентификатор VPN-соединения, если он был назначен.
    #[must_use]
    pub const fn id(&self) -> Option<VpnConnectionId> {
        self.id
    }

    /// Возвращает идентификатор пользователя.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Возвращает идентификатор VPN-ноды.
    #[must_use]
    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Проверяет, синхронизировано ли соединение с VPN-нодой.
    #[must_use]
    pub const fn is_synced(&self) -> bool {
        self.is_synced
    }

    /// Возвращает время создания VPN-соединения.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Отмечает VPN-соединение как синхронизированное с VPN-нодой.
    pub const fn mark_as_synced(&mut self) {
        self.is_synced = true;
    }

    /// Отмечает VPN-соединение как требующее синхронизации с VPN-нодой.
    pub const fn mark_as_unsynced(&mut self) {
        self.is_synced = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Days;
    use std::str::FromStr;

    // ==========================================
    // Node
    // ==========================================

    #[test]
    fn test_node_new_creates_active_node_without_id() {
        let ip = NodeIpAddress::from_str("198.51.100.5").unwrap();
        let node = Node::new("nl-1".to_string(), ip);

        assert_eq!(node.id(), None, "у новой ноды не должно быть ID до сохранения в БД");
        assert_eq!(node.name(), "nl-1");
        assert_eq!(node.ip_address(), ip);
        assert!(node.is_active(), "новая нода по умолчанию должна быть активной");
    }

    #[test]
    fn test_node_restore_from_db_and_getters() {
        let id = NodeId::new(5);
        let ip = NodeIpAddress::from_str("198.51.100.5").unwrap();
        let created_at = Utc::now() - Days::new(10);
        let node = Node::restore_from_db(id, "de-1".to_string(), ip, false, created_at);

        assert_eq!(node.id(), Some(id));
        assert_eq!(node.name(), "de-1");
        assert_eq!(node.ip_address(), ip);
        assert!(!node.is_active());
        assert_eq!(node.created_at(), created_at);
    }

    #[test]
    fn test_node_activate_sets_is_active_true() {
        let ip = NodeIpAddress::from_str("198.51.100.5").unwrap();
        let mut node =
            Node::restore_from_db(NodeId::new(1), "n".to_string(), ip, false, Utc::now());

        node.activate();

        assert!(node.is_active());
    }

    #[test]
    fn test_node_deactivate_sets_is_active_false() {
        let ip = NodeIpAddress::from_str("198.51.100.5").unwrap();
        let mut node = Node::new("n".to_string(), ip);

        node.deactivate();

        assert!(!node.is_active());
    }

    // ==========================================
    // VpnConnection
    // ==========================================

    #[test]
    fn test_vpn_connection_new_is_unsynced_and_without_id() {
        let conn = VpnConnection::new(UserId::new(1), NodeId::new(1));

        assert_eq!(conn.id(), None);
        assert_eq!(conn.user_id(), UserId::new(1));
        assert_eq!(conn.node_id(), NodeId::new(1));
        assert!(!conn.is_synced(), "новое соединение ещё не синхронизировано с нодой");
    }

    #[test]
    fn test_vpn_connection_restore_from_db_and_getters() {
        let id = VpnConnectionId::new(9);
        let created_at = Utc::now() - Days::new(3);
        let conn =
            VpnConnection::restore_from_db(id, UserId::new(2), NodeId::new(3), true, created_at);

        assert_eq!(conn.id(), Some(id));
        assert_eq!(conn.user_id(), UserId::new(2));
        assert_eq!(conn.node_id(), NodeId::new(3));
        assert!(conn.is_synced());
        assert_eq!(conn.created_at(), created_at);
    }

    #[test]
    fn test_vpn_connection_mark_as_synced() {
        let mut conn = VpnConnection::new(UserId::new(1), NodeId::new(1));

        conn.mark_as_synced();

        assert!(conn.is_synced());
    }

    #[test]
    fn test_vpn_connection_mark_as_unsynced() {
        let mut conn = VpnConnection::restore_from_db(
            VpnConnectionId::new(1),
            UserId::new(1),
            NodeId::new(1),
            true,
            Utc::now(),
        );

        conn.mark_as_unsynced();

        assert!(!conn.is_synced());
    }
}