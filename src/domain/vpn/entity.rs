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