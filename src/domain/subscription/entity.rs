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
    is_warning_sent: bool,
    created_at: DateTime<Utc>,
}

impl Subscription {
    /// Создаёт новую подписку.
    ///
    /// Идентификатор подписки устанавливается после сохранения в базе данных.
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
            is_warning_sent: false,
            created_at: Utc::now(),
        }
    }

    /// Восстанавливает подписку из базы данных.
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
        is_warning_sent: bool,
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
            is_warning_sent,
            created_at,
        }
    }

    /// Возвращает идентификатор подписки, если он был назначен.
    #[must_use]
    pub const fn id(&self) -> Option<SubscriptionId> {
        self.id
    }

    /// Возвращает идентификатор пользователя, которому принадлежит подписка.
    #[must_use]
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Возвращает тарифный план подписки.
    #[must_use]
    pub const fn plan(&self) -> SubscriptionPlan {
        self.plan
    }

    /// Возвращает время начала действия подписки.
    #[must_use]
    pub const fn starts_at(&self) -> DateTime<Utc> {
        self.starts_at
    }

    /// Возвращает время окончания действия подписки.
    #[must_use]
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Возвращает текущий статус подписки.
    #[must_use]
    pub const fn status(&self) -> SubscriptionStatus {
        self.status
    }

    /// Возвращает ограничения на количество устройств подписки.
    #[must_use]
    pub const fn devices(&self) -> SubscriptionDevices {
        self.devices
    }

    /// Возвращает признак того, было ли отправлено предупреждение о скором истечении подписки.
    #[must_use]
    pub const fn is_warning_sent(&self) -> bool {
        self.is_warning_sent
    }

    /// Возвращает время создания подписки.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Назначает подписке идентификатор.
    pub const fn assign_id(&mut self, id: SubscriptionId) {
        self.id = Some(id);
    }

    /// Проверяет, имеет ли подписка статус `Active`.
    #[must_use]
    pub fn can_extend(&self) -> bool {
        self.status == SubscriptionStatus::Active
    }

    /// Проверяет, истёк ли срок действия подписки.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Устанавливает подписке статус `Expired`.
    pub const fn expire(&mut self) {
        self.status = SubscriptionStatus::Expired;
    }

    /// Отмечает, что предупреждение о скором истечении подписки было отправлено
    pub const fn mark_warning_sent(&mut self) {
        self.is_warning_sent = true;
    }

    /// Проверяет, является ли подписка активной.
    ///
    /// Подписка считается активной, если она имеет статус `Active` и срок её действия ещё не истёк.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.can_extend() && !self.is_expired()
    }

    /// Возвращает количество полных дней до истечения срока действия.
    ///
    /// Для уже истёкшей подписки возвращает отрицательное значение.
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
        assert_eq!(subscription.is_warning_sent, false);
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
    fn test_expire_transitions_status_to_expired() {
        let mut sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );
        sub.expire();
        assert_eq!(
            sub.status,
            SubscriptionStatus::Expired,
            "expire() обязан переводить статус в Expired"
        );
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

    #[test]
    fn test_restore_from_db_and_getters() {
        let id = SubscriptionId::new(100500);
        let user_id = UserId::new(42);
        let plan = SubscriptionPlan::Month6;
        let starts_at = Utc::now() - Days::new(10);
        let expires_at = starts_at + Months::new(6);
        let status = SubscriptionStatus::Inactive;
        let is_warning_sent = false;
        let devices = SubscriptionDevices::new(5).unwrap();
        let created_at = Utc::now() - Days::new(10);

        let sub = Subscription::restore_from_db(
            id,
            user_id,
            plan,
            starts_at,
            expires_at,
            status,
            devices,
            is_warning_sent,
            created_at,
        );

        assert_eq!(sub.id(), Some(id));
        assert_eq!(sub.user_id(), user_id);
        assert_eq!(sub.plan(), plan);
        assert_eq!(sub.starts_at(), starts_at);
        assert_eq!(sub.expires_at(), expires_at);
        assert_eq!(sub.status(), status);
        assert_eq!(sub.devices(), devices);
        assert_eq!(sub.is_warning_sent(), is_warning_sent);
        assert_eq!(sub.created_at(), created_at);
    }

    #[test]
    fn test_assign_id_sets_expected_value() {
        let mut sub = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );

        assert_eq!(
            sub.id(),
            None,
            "У новой подписки не должно быть ID до сохранения в БД"
        );

        let expected_id = SubscriptionId::new(999);
        sub.assign_id(expected_id.clone());

        assert_eq!(
            sub.id(),
            Some(expected_id),
            "assign_id должен был присвоить корректный ID"
        );
    }

    #[test]
    fn test_mark_warning_sent_marks_warning_as_sent() {
        let mut subscription = create_test_subscription(
            SubscriptionPlan::Month3,
            SubscriptionStatus::Active,
            30,
        );

        assert!(
            !subscription.is_warning_sent(),
            "Для новой подписки предупреждение ещё не должно быть отправлено"
        );

        subscription.mark_warning_sent();

        assert!(
            subscription.is_warning_sent(),
            "Предупреждение должно считаться отправленным"
        );
    }
}
