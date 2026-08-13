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

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // --- UserId ---
    #[test]
    fn test_user_id() {
        let id_val = 42;
        let id = UserId::new(id_val);

        assert_eq!(id.inner(), id_val);
        assert_eq!(id.to_string(), "42");
    }

    // --- TelegramId ---
    #[test]
    fn test_telegram_id() {
        let tg_val = 123456789;
        let tg_id = TelegramId::new(tg_val);

        assert_eq!(tg_id.inner(), tg_val);
        assert_eq!(tg_id.to_string(), "123456789");
    }

    // --- ReferralCode ---
    #[test]
    fn test_referral_code() {
        let code_str = "PuCkAQB68".to_string();
        let ref_code = ReferralCode::new(code_str.clone());

        assert_eq!(ref_code.inner(), "PuCkAQB68");

        let generated1 = ReferralCode::generate();
        let generated2 = ReferralCode::generate();

        assert!(
            !generated1.inner().is_empty(),
            "Сгенерированный реф-код не должен быть пустым"
        );
        assert_ne!(
            generated1, generated2,
            "Два сгенерированных реф-кода должны различаться"
        );
    }

    // --- SubscriptionToken ---
    #[test]
    fn test_subscription_token() {
        let token_str = "0Md8B21SMCKX".to_string();
        let token = SubscriptionToken::new(token_str.clone());

        assert_eq!(token.inner(), "0Md8B21SMCKX");

        let generated1 = SubscriptionToken::generate();
        let generated2 = SubscriptionToken::generate();

        assert!(
            !generated1.inner().is_empty(),
            "Сгенерированный токен не должен быть пустым"
        );
        assert_ne!(
            generated1, generated2,
            "Два сгенерированных токена должны различаться"
        );
        assert_eq!(
            generated1.inner().len(),
            32,
            "Упрощенный UUID (simple) должен быть ровно 32 символа"
        );
    }

    // --- DiscountPercent ---
    #[test]
    fn test_discount_percent_valid() {
        assert_eq!(DiscountPercent::zero().inner(), 0);

        assert_eq!(DiscountPercent::new(0).unwrap().inner(), 0);
        assert_eq!(DiscountPercent::new(50).unwrap().inner(), 50);
        assert_eq!(DiscountPercent::new(100).unwrap().inner(), 100);
    }

    #[test]
    fn test_discount_percent_invalid() {
        let result_under = DiscountPercent::new(-1);
        assert!(result_under.is_err());
        assert!(matches!(
            result_under.unwrap_err(),
            DomainError::User(UserError::InvalidDiscount(-1))
        ));

        let result_over = DiscountPercent::new(101);
        assert!(result_over.is_err());
        assert!(matches!(
            result_over.unwrap_err(),
            DomainError::User(UserError::InvalidDiscount(101))
        ));
    }

    // --- Money ---
    #[test]
    fn test_money_valid() {
        let m1 = Money::new(0).unwrap();
        assert_eq!(m1.inner(), 0);
        assert_eq!(m1.to_string(), "0");

        let m2 = Money::new(10500).unwrap();
        assert_eq!(m2.inner(), 10500);
        assert_eq!(m2.to_string(), "10500");
    }

    #[test]
    fn test_money_invalid() {
        let result = Money::new(-1);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DomainError::User(UserError::InvalidMoney(-1))
        ));
    }

    // --- UserRole ---
    #[test]
    fn test_user_role_as_str() {
        assert_eq!(UserRole::User.as_str(), "user");
        assert_eq!(UserRole::Admin.as_str(), "admin");
    }

    #[test]
    fn test_user_role_from_str_valid() {
        assert_eq!(UserRole::from_str("user").unwrap(), UserRole::User);
        assert_eq!(UserRole::from_str("admin").unwrap(), UserRole::Admin);
    }

    #[test]
    fn test_user_role_from_str_invalid() {
        let result = UserRole::from_str("superuser");
        assert!(result.is_err());

        match result.unwrap_err() {
            DomainError::User(UserError::InvalidRole(role)) => { assert_eq!(role, "superuser") } _ => panic!("Ожидалась ошибка UserError::InvalidRole"),
        }
    }
}
