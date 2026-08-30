use crate::domain::error::NodeError;
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIpAddress(IpAddr);

impl NodeId {
    /// Создаёт идентификатор VPN-ноды из числового значения.
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl NodeIpAddress {
    #[must_use]
    pub const fn inner(&self) -> IpAddr {
        self.0
    }
}

impl Display for NodeIpAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NodeIpAddress {
    type Err = NodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ip = s
            .parse::<IpAddr>()
            .map_err(|_| NodeError::InvalidIpAddress(s.to_string()))?;
        Ok(Self(ip))
    }
}

// ============================================================================================== //

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VpnConnectionId(i64);

impl VpnConnectionId {
    /// Создаёт идентификатор VPN-соединения из числового значения.
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for VpnConnectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::str::FromStr;

    // --- NodeId ---
    #[test]
    fn test_node_id() {
        let id_val = 42;
        let node_id = NodeId::new(id_val);

        assert_eq!(node_id.inner(), id_val);
        assert_eq!(node_id.to_string(), "42");
    }

    // --- NodeIpAddress ---
    #[test]
    fn test_node_ip_address_from_str_valid_ipv4() {
        let ip = NodeIpAddress::from_str("203.0.113.10").unwrap();

        assert_eq!(ip.inner(), "203.0.113.10".parse::<IpAddr>().unwrap());
        assert_eq!(ip.to_string(), "203.0.113.10");
    }

    #[test]
    fn test_node_ip_address_from_str_valid_ipv6() {
        let ip = NodeIpAddress::from_str("2001:db8::1").unwrap();

        assert_eq!(ip.inner(), "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_node_ip_address_from_str_invalid() {
        let result = NodeIpAddress::from_str("это-не-айпи");

        assert!(result.is_err());
        match result.unwrap_err() {
            NodeError::InvalidIpAddress(s) => assert_eq!(s, "это-не-айпи"),
        }
    }

    // --- VpnConnectionId ---
    #[test]
    fn test_vpn_connection_id() {
        let id_val = 777;
        let conn_id = VpnConnectionId::new(id_val);

        assert_eq!(conn_id.inner(), id_val);
        assert_eq!(conn_id.to_string(), "777");
    }
}