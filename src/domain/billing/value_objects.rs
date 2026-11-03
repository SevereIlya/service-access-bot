use crate::domain::{BillingError, DomainError, DomainResult};
use std::fmt::{Display, Formatter};
use strum::{EnumString, IntoStaticStr, VariantArray};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId(i64);

impl TransactionId {
    /// Создаёт идентификатор транзакции из значения.
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, strum::Display, EnumString, IntoStaticStr, VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = DomainError,
    parse_err_fn = TransactionKind::parse_err,
)]
pub enum TransactionKind {
    /// Пополнение через платёжного провайдера
    TopUp,

    /// Списание за тариф
    Purchase,

    /// Кэшбек рефереру с покупки его реферала
    ReferralCashback,

    /// Разовый подарок приглашённому при активации реферальной ссылки
    ReferralWelcomeBonus,

    /// Возврат средств
    Refund,
}

impl TransactionKind {
    fn parse_err(value: &str) -> DomainError {
        BillingError::InvalidTransactionKind(value.to_owned()).into()
    }

    #[must_use]
    pub const fn is_credit(&self) -> bool {
        matches!(
            self,
            Self::TopUp | Self::ReferralCashback | Self::ReferralWelcomeBonus | Self::Refund
        )
    }

    #[must_use]
    pub fn credit_kind_strings() -> Vec<&'static str> {
        Self::VARIANTS.iter().filter(|kind| kind.is_credit()).map(Into::into).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, EnumString, IntoStaticStr)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = DomainError,
    parse_err_fn = TransactionStatus::parse_err
)]
pub enum TransactionStatus {
    Pending,
    Success,
    Failed,
}

impl TransactionStatus {
    fn parse_err(value: &str) -> DomainError {
        BillingError::InvalidTransactionStatus(value.to_owned()).into()
    }
}

/// `Internal` для операций без внешнего провайдера (списание за тариф, реферальный кэшбек)
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, EnumString, IntoStaticStr)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = DomainError,
    parse_err_fn = PaymentProvider::parse_err
)]
pub enum PaymentProvider {
    Platega,
    Internal,
}

impl PaymentProvider {
    fn parse_err(value: &str) -> DomainError {
        BillingError::InvalidPaymentProvider(value.to_owned()).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Money(i64);

impl Money {
    pub fn new(value: i64) -> DomainResult<Self> {
        if value < 0 {
            return Err(BillingError::InvalidAmount(value).into());
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Значение в копейках как `f64`.
    #[must_use]
    pub const fn as_kopecks_f64(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        {
            self.0 as f64
        }
    }

    /// Значение в рублях как `f64`.
    #[must_use]
    pub fn as_rubles_f64(&self) -> f64 {
        self.as_kopecks_f64() / 100.0
    }

    /// Округляет вверх до целого рубля.
    #[must_use]
    pub const fn ceil_to_rubles(&self) -> Self {
        Self(((self.0 + 99) / 100) * 100)
    }

    /// Парсит рубли как `f64` от ВНЕШНЕГО источника (провайдер) в копейки.
    /// Здесь данные не достоверные отсюда проверки.
    ///
    /// # Errors
    /// Возвращает ошибку, если `rubles` не конечное число
    /// или выходит за пределы точного представления в `f64`.
    pub fn try_from_rubles_f64(rubles: f64) -> DomainResult<Self> {
        const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0; // 2^53 - 1

        if !rubles.is_finite() {
            return Err(BillingError::NonFiniteAmount.into());
        }

        let kopecks: f64 = (rubles * 100.0).round();

        if !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&kopecks) {
            return Err(BillingError::AmountOutOfRange(rubles).into());
        }

        #[allow(clippy::cast_possible_truncation)]
        let kopecks_i64: i64 = kopecks as i64;

        Self::new(kopecks_i64)
    }
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
            DomainError::Billing(BillingError::InvalidAmount(-1))
        ));
    }

    #[test]
    fn test_ceil_to_rubles_rounds_up_to_whole_ruble() {
        assert_eq!(
            Money::new(26_673).unwrap().ceil_to_rubles().inner(),
            26_700,
            "266.73 -> 267.00"
        );
        assert_eq!(
            Money::new(26_700).unwrap().ceil_to_rubles().inner(),
            26_700,
            "уже целое - не трогаем"
        );
        assert_eq!(Money::new(0).unwrap().ceil_to_rubles().inner(), 0);
        assert_eq!(
            Money::new(1).unwrap().ceil_to_rubles().inner(),
            100,
            "1 копейка всё равно округляется до целого рубля"
        );
    }

    // --- TransactionKind ---
    #[test]
    fn test_kind_str_roundtrip_for_every_variant() {
        for &kind in TransactionKind::VARIANTS {
            let string_repr: &'static str = kind.into();
            let parsed: TransactionKind = string_repr.parse().unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn test_kind_unknown_str_is_rejected_with_correct_error() {
        let bad_str = "crypto";
        let result: Result<TransactionKind, DomainError> = bad_str.parse();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DomainError::Billing(BillingError::InvalidTransactionKind(s)) if s == bad_str
        ));
    }

    #[test]
    fn test_kind_variants_have_unique_names() {
        let names: HashSet<&str> = TransactionKind::VARIANTS.iter().map(Into::into).collect();
        assert_eq!(names.len(), TransactionKind::VARIANTS.len());
    }

    #[test]
    fn test_kind_credit_classification_spec() {
        for kind in [
            TransactionKind::TopUp,
            TransactionKind::ReferralCashback,
            TransactionKind::ReferralWelcomeBonus,
            TransactionKind::Refund,
        ] {
            assert!(kind.is_credit(), "{kind:?} должен пополнять баланс");
        }
        assert!(!TransactionKind::Purchase.is_credit(), "покупка списывает");
    }

    #[test]
    fn test_kind_persisted_names_are_pinned() {
        let pinned = [
            (TransactionKind::TopUp, "top_up"),
            (TransactionKind::Purchase, "purchase"),
            (TransactionKind::ReferralCashback, "referral_cashback"),
            (
                TransactionKind::ReferralWelcomeBonus,
                "referral_welcome_bonus",
            ),
            (TransactionKind::Refund, "refund"),
        ];

        assert_eq!(
            pinned.len(),
            TransactionKind::VARIANTS.len(),
            "добавил вариант - закрепи его имя здесь и в CHECK миграции"
        );
        for (kind, expected) in pinned {
            assert_eq!(<&'static str>::from(kind), expected);
        }
    }

    #[test]
    fn test_kind_credit_kind_strings_are_exactly_the_credits() {
        let credit_strs = TransactionKind::credit_kind_strings();

        assert_eq!(credit_strs.len(), 4);

        for string in credit_strs {
            let parsed: TransactionKind = string.parse().unwrap();
            assert!(
                parsed.is_credit(),
                "Строка {string} не является кредитной операцией"
            );
        }
    }
}
