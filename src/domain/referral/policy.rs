use crate::domain::{billing::Money, referral::CashbackPercent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferralPolicy {
    welcome_bonus: Money,
    cashback: CashbackPercent,
}

impl ReferralPolicy {
    #[must_use]
    pub const fn new(welcome_bonus: Money, cashback: CashbackPercent) -> Self {
        Self {
            welcome_bonus,
            cashback,
        }
    }

    /// 0 значит бонус выключен
    #[must_use]
    pub const fn welcome_bonus(&self) -> Money {
        self.welcome_bonus
    }

    #[must_use]
    pub const fn cashback_percent(&self) -> CashbackPercent {
        self.cashback
    }

    #[must_use]
    pub fn cashback_for(&self, price: Money) -> Money {
        self.cashback.of(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(bonus_kopecks: i64, percent: i32) -> ReferralPolicy {
        ReferralPolicy::new(
            Money::new(bonus_kopecks).unwrap(),
            CashbackPercent::new(percent).unwrap(),
        )
    }

    #[test]
    fn test_policy_exposes_configured_values() {
        let policy = policy(3_800, 10);

        assert_eq!(policy.welcome_bonus().inner(), 3_800);
        assert_eq!(policy.cashback_percent().inner(), 10);
    }

    #[test]
    fn test_policy_cashback_for_cheapest_tariff() {
        // 58 ₽ * 10% = 5.80 ₽
        let cashback = policy(3_800, 10).cashback_for(Money::new(5_800).unwrap());

        assert_eq!(cashback.inner(), 580);
    }

    #[test]
    fn test_policy_zero_percent_disables_cashback() {
        let cashback = policy(3_800, 0).cashback_for(Money::new(5_800).unwrap());

        assert!(cashback.is_zero());
    }
}
