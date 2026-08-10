use crate::domain::error::{DomainError, DomainResult, UserError};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelegramId(i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferralCode(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionToken(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiscountPercent(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    User,
    Admin,
}

// ============================================================================================== //

impl UserId {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TelegramId {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for TelegramId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ReferralCode {
    #[must_use]
    pub const fn new(code: String) -> Self {
        Self(code)
    }

    #[must_use]
    pub fn generate() -> Self {
        let uuid = Uuid::new_v4();

        #[allow(clippy::cast_possible_truncation)]
        let short_id = uuid.as_u128() as u64;

        Self(base62::encode(short_id))
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl SubscriptionToken {
    #[must_use]
    pub const fn new(token: String) -> Self {
        Self(token)
    }

    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl DiscountPercent {
    pub fn new(value: i32) -> DomainResult<Self> {
        if !(0..=100).contains(&value) {
            return Err(UserError::InvalidDiscount(value).into());
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        Ok(Self(value as u8))
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn inner(&self) -> i32 {
        self.0 as i32
    }
}

impl Money {
    pub fn new(value: i64) -> DomainResult<Self> {
        if value < 0 {
            return Err(UserError::InvalidMoney(value).into());
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
            _ => Err(UserError::InvalidRole(s.to_string()).into()),
        }
    }
}
