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
