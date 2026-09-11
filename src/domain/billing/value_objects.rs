use crate::domain::{BillingError, DomainError, DomainResult};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId(i64);

impl TransactionId {
    /// Создаёт идентификатор подписки из значения.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    TopUp,
    Purchase,
    ReferralCashback,
    Refund,
}

impl TransactionKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::TopUp => "top_up",
            Self::Purchase => "purchase",
            Self::ReferralCashback => "referral_cashback",
            Self::Refund => "refund",
        }
    }

    #[must_use]
    pub const fn is_credit(&self) -> bool {
        matches!(self, Self::TopUp | Self::ReferralCashback | Self::Refund)
    }
}

impl FromStr for TransactionKind {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "top_up" => Ok(Self::TopUp),
            "purchase" => Ok(Self::Purchase),
            "referral_cashback" => Ok(Self::ReferralCashback),
            "refund" => Ok(Self::Refund),
            _ => Err(BillingError::InvalidTransactionKind(s.to_string()).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Pending,
    Success,
    Failed,
}

impl TransactionStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for TransactionStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            _ => Err(BillingError::InvalidTransactionStatus(s.to_string()).into()),
        }
    }
}

/// `Internal` для операций без внешнего провайдера (списание за тариф, реферальный кэшбек)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentProvider {
    Platega,
    Internal,
}

impl PaymentProvider {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Platega => "platega",
            Self::Internal => "internal",
        }
    }
}

impl FromStr for PaymentProvider {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "platega" => Ok(Self::Platega),
            "internal" => Ok(Self::Internal),
            _ => Err(BillingError::InvalidPaymentProvider(s.to_string()).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
