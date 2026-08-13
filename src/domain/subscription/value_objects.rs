use crate::domain::error::{DomainError, DomainResult, SubscriptionError};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(i64);

/// Тарифный план подписки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionPlan {
    Trial,
    Month1,
    Month3,
    Month6,
    Month12,
}

/// Текущий статус подписки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionStatus {
    Active,
    Inactive,
    Expired,
    Canceled,
}

/// Количество устройств, доступных в рамках подписки.
///
/// Допустимое количество устройств — от 1 до 10 включительно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionDevices(i32);

// =============================================================================================

impl SubscriptionId {
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

impl Display for SubscriptionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SubscriptionPlan {
    /// Возвращает строковое представление тарифного плана.
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

    /// Создаёт тарифный план из его строкового представления.
    ///
    /// Возвращает [`SubscriptionError::InvalidPlan`], если строка
    /// не соответствует ни одному из доступных тарифных планов.
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
    /// Возвращает строковое представление статуса подписки.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Expired => "expired",
            Self::Canceled => "canceled",
        }
    }
}

impl FromStr for SubscriptionStatus {
    type Err = DomainError;

    /// Создаёт статус подписки из его строкового представления.
    ///
    /// Возвращает [`SubscriptionError::InvalidStatus`], если строка
    /// не соответствует ни одному из доступных статусов.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "expired" => Ok(Self::Expired),
            "canceled" => Ok(Self::Canceled),
            _ => Err(SubscriptionError::InvalidStatus(s.to_string()).into()),
        }
    }
}

impl SubscriptionDevices {
    /// Создаёт количество устройств.
    ///
    /// Возвращает ошибку, если значение находится вне диапазона от 1 до 10.
    pub fn new(value: i32) -> DomainResult<Self> {
        if !(1..=10).contains(&value) {
            return Err(SubscriptionError::InvalidDevices(value).into());
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

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // --- SubscriptionId ---
    #[test]
    fn test_subscription_id() {
        let id_val = 999;
        let sub_id = SubscriptionId::new(id_val);

        assert_eq!(sub_id.inner(), id_val);
        assert_eq!(sub_id.to_string(), "999");
    }

    // --- SubscriptionPlan ---
    #[test]
    fn test_subscription_plan_as_str() {
        assert_eq!(SubscriptionPlan::Trial.as_str(), "trial");
        assert_eq!(SubscriptionPlan::Month1.as_str(), "month_1");
        assert_eq!(SubscriptionPlan::Month3.as_str(), "month_3");
        assert_eq!(SubscriptionPlan::Month6.as_str(), "month_6");
        assert_eq!(SubscriptionPlan::Month12.as_str(), "month_12");
    }

    #[test]
    fn test_subscription_plan_from_str_valid() {
        assert_eq!(
            SubscriptionPlan::from_str("trial").unwrap(),
            SubscriptionPlan::Trial
        );
        assert_eq!(
            SubscriptionPlan::from_str("month_1").unwrap(),
            SubscriptionPlan::Month1
        );
        assert_eq!(
            SubscriptionPlan::from_str("month_3").unwrap(),
            SubscriptionPlan::Month3
        );
        assert_eq!(
            SubscriptionPlan::from_str("month_6").unwrap(),
            SubscriptionPlan::Month6
        );
        assert_eq!(
            SubscriptionPlan::from_str("month_12").unwrap(),
            SubscriptionPlan::Month12
        );
    }

    #[test]
    fn test_subscription_plan_from_str_invalid() {
        let result = SubscriptionPlan::from_str("forever");
        assert!(result.is_err());

        match result.unwrap_err() {
            DomainError::Subscription(SubscriptionError::InvalidPlan(plan)) => { assert_eq!(plan, "forever") } _ => panic!("Ожидалась ошибка SubscriptionError::InvalidPlan"),
        }
    }

    // --- SubscriptionStatus ---
    #[test]
    fn test_subscription_status_as_str() {
        assert_eq!(SubscriptionStatus::Active.as_str(), "active");
        assert_eq!(SubscriptionStatus::Inactive.as_str(), "inactive");
        assert_eq!(SubscriptionStatus::Expired.as_str(), "expired");
        assert_eq!(SubscriptionStatus::Canceled.as_str(), "canceled");
    }

    #[test]
    fn test_subscription_status_from_str_valid() {
        assert_eq!(
            SubscriptionStatus::from_str("active").unwrap(),
            SubscriptionStatus::Active
        );
        assert_eq!(
            SubscriptionStatus::from_str("inactive").unwrap(),
            SubscriptionStatus::Inactive
        );
        assert_eq!(
            SubscriptionStatus::from_str("expired").unwrap(),
            SubscriptionStatus::Expired
        );
        assert_eq!(
            SubscriptionStatus::from_str("canceled").unwrap(),
            SubscriptionStatus::Canceled
        );
    }

    #[test]
    fn test_subscription_status_from_str_invalid() {
        let result = SubscriptionStatus::from_str("banned");
        assert!(result.is_err());

        match result.unwrap_err() {
            DomainError::Subscription(SubscriptionError::InvalidStatus(status)) => { assert_eq!(status, "banned") } _ => panic!("Ожидалась ошибка SubscriptionError::InvalidStatus"),
        }
    }

    // --- SubscriptionDevices ---
    #[test]
    fn test_subscription_devices_valid() {
        let dev_min = SubscriptionDevices::new(1).unwrap();
        assert_eq!(dev_min.inner(), 1);
        assert_eq!(dev_min.to_string(), "1");

        let dev_mid = SubscriptionDevices::new(5).unwrap();
        assert_eq!(dev_mid.inner(), 5);

        let dev_max = SubscriptionDevices::new(10).unwrap();
        assert_eq!(dev_max.inner(), 10);
    }

    #[test]
    fn test_subscription_devices_invalid() {
        let result_under = SubscriptionDevices::new(0);
        assert!(result_under.is_err());
        match result_under.unwrap_err() {
            DomainError::Subscription(SubscriptionError::InvalidDevices(val)) => { assert_eq!(val, 0) } _ => panic!("Ожидалась ошибка SubscriptionError::InvalidDevices"),
        }

        let result_over = SubscriptionDevices::new(11);
        assert!(result_over.is_err());
        match result_over.unwrap_err() {
            DomainError::Subscription(SubscriptionError::InvalidDevices(val)) => { assert_eq!(val, 11) } _ => panic!("Ожидалась ошибка SubscriptionError::InvalidDevices"),
        }
    }
}
