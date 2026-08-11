use crate::domain::subscription::{
    SubscriptionDevices, SubscriptionId, SubscriptionPlan, SubscriptionStatus,
};
use crate::domain::user::UserId;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Subscription {
    id: Option<SubscriptionId>,
    user_id: UserId,
    plan: SubscriptionPlan,
    starts_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    status: SubscriptionStatus,
    devices: SubscriptionDevices,
    created_at: DateTime<Utc>,
}

impl Subscription {
    #[must_use]
    pub fn new(
        user_id: UserId,
        plan: SubscriptionPlan,
        starts_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        status: SubscriptionStatus,
        devices: SubscriptionDevices,
    ) -> Self {
        Self {
            id: None,
            user_id,
            plan,
            starts_at,
            expires_at,
            status,
            devices,
            created_at: Utc::now(),
        }
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn restore_from_db(
        id: SubscriptionId,
        user_id: UserId,
        plan: SubscriptionPlan,
        starts_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        status: SubscriptionStatus,
        devices: SubscriptionDevices,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            user_id,
            plan,
            starts_at,
            expires_at,
            status,
            devices,
            created_at,
        }
    }

    pub const fn assign_id(&mut self, id: SubscriptionId) {
        self.id = Some(id);
    }

    #[must_use]
    pub const fn id(&self) -> Option<SubscriptionId> {
        self.id
    }

    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    #[must_use]
    pub const fn plan(&self) -> SubscriptionPlan {
        self.plan
    }

    #[must_use]
    pub const fn starts_at(&self) -> DateTime<Utc> {
        self.starts_at
    }

    #[must_use]
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    #[must_use]
    pub const fn status(&self) -> SubscriptionStatus {
        self.status
    }

    #[must_use]
    pub const fn devices(&self) -> SubscriptionDevices {
        self.devices
    }

    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Проверяет, может ли подписка быть продлена
    #[must_use]
    pub fn can_extend(&self) -> bool {
        self.status == SubscriptionStatus::Active
    }

    /// Проверяет, истекла ли подписка
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Проверяет, активна ли подписка (не истекла и статус active)
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.can_extend() && !self.is_expired()
    }

    /// Возвращает количество дней до истечения подписки
    #[must_use]
    pub fn days_until_expiry(&self) -> i64 {
        (self.expires_at - Utc::now()).num_days()
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Days, Months};

    // ==========================================
    // ВСПОМОГАТЕЛЬНАЯ ФУНКЦИЯ
    // ==========================================

    fn create_test_subscription(
        plan: SubscriptionPlan,
        status: SubscriptionStatus,
        days_until_expiry: u64,
    ) -> Subscription {
        let now = Utc::now();
        Subscription::new(
            UserId::new(42),
            plan,
            now,
            now + Days::new(days_until_expiry),
            status,
            SubscriptionDevices::new(2).unwrap(),
        )
    }

    // ==========================================
    // ТЕСТЫ
    // ==========================================

    #[test]
    fn test_subscription_new_creates_valid_default_subscription() {
        let user_id = UserId::new(42);
        let plan = SubscriptionPlan::Month3;
        let starts_at = Utc::now();
        let expires_at = starts_at + Months::new(3);
        let status = SubscriptionStatus::Active;
        let devices = SubscriptionDevices::new(2).unwrap();
        let subscription = Subscription::new(
            user_id,
            plan.clone(),
            starts_at,
            expires_at,
            status.clone(),
            devices,
        );
        assert_eq!(subscription.id, None);
        assert_eq!(subscription.user_id, user_id);
        assert_eq!(subscription.plan, plan);
        assert_eq!(subscription.starts_at, starts_at);
        assert_eq!(subscription.expires_at, expires_at);
        assert_eq!(subscription.status, status);
        assert_eq!(subscription.devices, devices);
    }

    #[test]
    fn test_is_expired_returns_false_for_active_subscription() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );
        assert!(!sub.is_expired(), "Подписка не должна быть истекшей");
    }

    #[test]
    fn test_is_expired_returns_true_for_expired_subscription() {
        let now = Utc::now();
        let sub = Subscription::new(
            UserId::new(42),
            SubscriptionPlan::Month3,
            now - Days::new(10),
            now - Days::new(1),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(2).unwrap(),
        );
        assert!(sub.is_expired(), "Подписка должна быть истекшей");
    }

    #[test]
    fn test_is_active_returns_true_for_active_non_expired() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );
        assert!(sub.is_active(), "Подписка должна быть активной");
    }

    #[test]
    fn test_is_active_returns_false_for_inactive_status() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Inactive,
            30,
        );
        assert!(
            !sub.is_active(),
            "Неактивная подписка не должна быть активной"
        );
    }

    #[test]
    fn test_is_active_returns_false_for_expired_subscription() {
        let now = Utc::now();
        let sub = Subscription::new(
            UserId::new(42),
            SubscriptionPlan::Month3,
            now - Days::new(10),
            now - Days::new(1),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(2).unwrap(),
        );
        assert!(
            !sub.is_active(),
            "Истекшая подписка не должна быть активной"
        );
    }

    #[test]
    fn test_can_extend_returns_true_for_active_status() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );
        assert!(
            sub.can_extend(),
            "Активная подписка должна быть продлеваемой"
        );
    }

    #[test]
    fn test_can_extend_returns_false_for_inactive_status() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Inactive,
            30,
        );
        assert!(
            !sub.can_extend(),
            "Неактивная подписка не должна быть продлеваемой"
        );
    }

    #[test]
    fn test_can_extend_returns_false_for_canceled_status() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Canceled,
            30,
        );
        assert!(
            !sub.can_extend(),
            "Отменённая подписка не должна быть продлеваемой"
        );
    }

    #[test]
    fn test_days_until_expiry_returns_correct_value() {
        let sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            5,
        );
        let days = sub.days_until_expiry();
        assert!(
            days >= 4 && days <= 5,
            "Должно быть примерно 5 дней до истечения"
        );
    }

    #[test]
    fn test_days_until_expiry_returns_negative_for_expired() {
        let now = Utc::now();
        let sub = Subscription::new(
            UserId::new(42),
            SubscriptionPlan::Month3,
            now - Days::new(10),
            now - Days::new(1),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(2).unwrap(),
        );
        let days = sub.days_until_expiry();
        assert!(
            days < 0,
            "Для истекшей подписки количество дней должно быть отрицательным"
        );
    }
}
