use crate::domain::error::DomainError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelegramId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferralCode(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionToken(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiscountPercent(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    User,
    Admin,
}

// =============================================================================================

impl UserRole {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
        }
    }
}

impl FromStr for UserRole {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "admin" => Ok(Self::Admin),
            _ => Err(DomainError::InvalidRole(s.to_string())),
        }
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for TelegramId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ReferralCode {
    #[must_use]
    pub fn generate() -> Self {
        let uuid = Uuid::new_v4();

        #[allow(clippy::cast_possible_truncation)]
        let short_id = uuid.as_u128() as u64;

        Self(base62::encode(short_id))
    }
}

impl SubscriptionToken {
    #[must_use]
    pub fn generate() -> Self {
        let uuid = Uuid::new_v4();
        Self(uuid.simple().to_string())
    }
}

impl DiscountPercent {
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn new(value: i32) -> Self {
        Self(value.clamp(0, 100) as u8)
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }
}
