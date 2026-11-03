use crate::domain::{DomainError, DomainResult, UserError};
use std::fmt::{Display, Formatter};
use strum::{EnumString, IntoStaticStr, VariantArray};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(i64);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelegramId(i64);

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferralCode(String);

impl ReferralCode {
    const MAX_LEN: usize = 64;

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

    /// Разбирает код из payload `/start`
    ///
    /// Доступны только `A-Z a-z 0-9 _ -`, длина `1..=64`
    ///
    /// # Errors
    /// [`UserError::InvalidReferralCode`], если ввод пуст, слишком длинный или содержит посторонние символы.
    pub fn parse(raw: &str) -> DomainResult<Self> {
        let is_valid = (1..=Self::MAX_LEN).contains(&raw.len())
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
        if is_valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(UserError::InvalidReferralCode.into())
        }
    }

    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionToken(String);

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

/// Информация о пользователе другому пользователю.
/// Намеренно без `telegram_id`.
/// `Username` хранится без `@`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicIdentity {
    Username(String),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiscountPercent(u8);

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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, strum::Display, EnumString, IntoStaticStr, VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = DomainError,
    parse_err_fn = UserRole::parse_err,
)]
pub enum UserRole {
    User,
    Admin,
}

impl UserRole {
    fn parse_err(value: &str) -> DomainError {
        UserError::InvalidRole(value.to_owned()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let tg_val = 123_456_789;
        let tg_id = TelegramId::new(tg_val);

        assert_eq!(tg_id.inner(), tg_val);
        assert_eq!(tg_id.to_string(), "123456789");
    }

    // --- ReferralCode ---
    #[test]
    fn test_referral_code() {
        let code_str = "PuCkAQB68".to_string();
        let ref_code = ReferralCode::new(code_str);

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

    #[test]
    fn test_referral_code_parse_accepts_generated_codes() {
        for _ in 0..100 {
            let generated = ReferralCode::generate();
            assert_eq!(ReferralCode::parse(generated.inner()).unwrap(), generated);
        }
    }

    #[test]
    fn test_referral_code_parse_accepts_telegram_charset_and_max_len() {
        assert!(ReferralCode::parse("aZ09_-").is_ok());
        assert!(ReferralCode::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn test_referral_code_parse_rejects_garbage() {
        let too_long = "a".repeat(65);
        let cases = [
            "",
            too_long.as_str(),
            "has space",
            "semi;colon",
            "quote'",
            "кириллица",
            "emoji🎁",
            "line\nbreak",
            "%20",
        ];

        for bad in cases {
            let err = ReferralCode::parse(bad).unwrap_err();
            assert!(
                matches!(err, DomainError::User(UserError::InvalidReferralCode)),
                "{bad:?} должен быть отклонён"
            );
        }
    }

    // --- SubscriptionToken ---
    #[test]
    fn test_subscription_token() {
        let token_str = "0Md8B21SMCKX".to_string();
        let token = SubscriptionToken::new(token_str);

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

    // --- UserRole ---
    #[test]
    fn test_role_str_roundtrip() {
        for &role in UserRole::VARIANTS {
            let s: &'static str = role.into();
            let parsed: UserRole = s.parse().unwrap();
            assert_eq!(parsed, role);
        }
    }

    #[test]
    fn test_role_unknown_rejected() {
        let bad = "superadmin";
        let result: Result<UserRole, DomainError> = bad.parse();
        assert!(matches!(
            result.unwrap_err(),
            DomainError::User(UserError::InvalidRole(s)) if s == bad
        ));
    }

    #[test]
    fn test_role_pinned_names() {
        let pinned = [(UserRole::User, "user"), (UserRole::Admin, "admin")];

        assert_eq!(pinned.len(), UserRole::VARIANTS.len());
        for (role, expected) in pinned {
            assert_eq!(<&'static str>::from(role), expected);
        }
    }
}
