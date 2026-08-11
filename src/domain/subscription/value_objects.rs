use crate::domain::error::{DomainError, DomainResult, SubscriptionError};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(i64);

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
pub struct SubscriptionDevices(i32);

// =============================================================================================

impl SubscriptionId {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for SubscriptionId {
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
            _ => Err(SubscriptionError::InvalidPlan(s.to_string()).into()),
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
            _ => Err(SubscriptionError::InvalidStatus(s.to_string()).into()),
        }
    }
}

impl SubscriptionDevices {
    #[must_use]
    pub fn new(value: i32) -> DomainResult<Self> {
        if !(1..=10).contains(&value) {
            return Err(SubscriptionError::InvalidDevices(value).into())
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn inner(&self) -> i32 {
        self.0
    }
}

impl Display for SubscriptionDevices {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
