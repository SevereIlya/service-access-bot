use crate::domain::{DomainResult, ReferralError, billing::Money};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferralId(i64);

impl ReferralId {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn inner(&self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for ReferralId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Процент кэшбека рефереру: целое число `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashbackPercent(u8);

impl CashbackPercent {
    /// # Errors
    /// [`ReferralError::InvalidCashbackPercent`], если значение вне `0..=100`.
    pub fn new(value: i32) -> DomainResult<Self> {
        match u8::try_from(value) {
            Ok(percent) if percent <= 100 => Ok(Self(percent)),
            _ => Err(ReferralError::InvalidCashbackPercent(value).into()),
        }
    }

    #[must_use]
    pub const fn inner(&self) -> u8 {
        self.0
    }

    /// Доля от суммы, округленная до копейки вниз.
    ///
    /// Для защиты от переполнения промежуточно считаем в 128-бит.
    ///
    /// # Panics
    /// Паника невозможна, поскольку `amount >= 0` и `percent 0..=100`.
    /// Итоговая сумма всегда неотрицательна.
    #[must_use]
    pub fn of(&self, amount: Money) -> Money {
        let percent = i128::from(self.0);
        let amount_i128 = i128::from(amount.inner());

        #[allow(clippy::cast_possible_truncation)]
        let result = ((amount_i128 * percent) / 100) as i64;

        Money::new(result).expect("Кэшбек не может быть отрицательным")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainError;

    fn cashback(percent: i32, kopecks: i64) -> i64 {
        CashbackPercent::new(percent).unwrap().of(Money::new(kopecks).unwrap()).inner()
    }

    // --- ReferralId ---
    #[test]
    fn test_referral_id() {
        let id = ReferralId::new(42);

        assert_eq!(id.inner(), 42);
        assert_eq!(id.to_string(), "42");
    }

    // --- CashbackPercent::new ---
    #[test]
    fn test_cashback_percent_accepts_bounds() {
        for valid in [0, 1, 10, 99, 100] {
            assert_eq!(
                CashbackPercent::new(valid).unwrap().inner(),
                u8::try_from(valid).unwrap()
            );
        }
    }

    #[test]
    fn test_cashback_percent_rejects_out_of_range() {
        for bad in [-1, 101, 255, 256, i32::MAX, i32::MIN] {
            let err = CashbackPercent::new(bad).unwrap_err();
            assert!(
                matches!(
                    err,
                    DomainError::Referral(ReferralError::InvalidCashbackPercent(v)) if v == bad
                ),
                "{bad} должен быть отклонён"
            );
        }
    }

    // --- CashbackPercent::of ---
    #[test]
    fn test_cashback_of_rounds_down() {
        assert_eq!(
            cashback(10, 5_761),
            576,
            "57.61 ₽ * 10% = 5.761 ₽ -> 5.76 ₽"
        );
        assert_eq!(cashback(10, 5_800), 580);
        assert_eq!(cashback(10, 99), 9, "9.9 коп. -> 9 коп.");
        assert_eq!(cashback(10, 9), 0, "0.9 коп. -> 0 коп.");
        assert_eq!(cashback(33, 199), 65, "65.67 коп. -> 65 коп.");
    }

    #[test]
    fn test_cashback_of_edges() {
        assert_eq!(cashback(0, 123_456), 0);
        assert_eq!(cashback(100, 123_456), 123_456);
        assert_eq!(cashback(10, 0), 0);
    }

    #[test]
    fn test_cashback_of_does_not_overflow_on_huge_amounts() {
        assert_eq!(cashback(100, i64::MAX), i64::MAX);
    }
}
