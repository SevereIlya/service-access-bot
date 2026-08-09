use crate::domain::error::DomainError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionPlan {
    Trial,
    Month1,
    Month3,
    Month6,
    Month12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionStatus {
    Active,
    Inactive,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionDevices(pub i32);

// =============================================================================================

impl Display for SubscriptionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for SubscriptionDevices {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SubscriptionPlan {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Trial => "trial",
            Self::Month1 => "month_1",
            Self::Month3 => "month_3",
            Self::Month6 => "month_6",
            Self::Month12 => "month_12",
        }
    }
}

impl FromStr for SubscriptionPlan {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trial" => Ok(Self::Trial),
            "month_1" => Ok(Self::Month1),
            "month_3" => Ok(Self::Month3),
            "month_6" => Ok(Self::Month6),
            "month_12" => Ok(Self::Month12),
            _ => Err(DomainError::InvalidPlan(s.to_string())),
        }
    }
}

impl SubscriptionStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Canceled => "canceled",
        }
    }
}

impl FromStr for SubscriptionStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "canceled" => Ok(Self::Canceled),
            _ => Err(DomainError::InvalidStatus(s.to_string())),
        }
    }
}
