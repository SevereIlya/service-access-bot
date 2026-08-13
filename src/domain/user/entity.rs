use crate::domain::error::{DomainError, DomainResult, UserError};
use crate::domain::user::{
    DiscountPercent, Money, ReferralCode, SubscriptionToken, TelegramId, UserId, UserRole,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    id: Option<UserId>,
    telegram_id: TelegramId,
    uuid: Uuid,
    username: Option<String>,
    full_name: String,
    role: UserRole,
    frozen_base_price: Money,
    referral_code: ReferralCode,
    subscription_token: SubscriptionToken,
    trial_used: bool,
    discount_percent: DiscountPercent,
    created_at: DateTime<Utc>,
}

impl User {
    #[must_use]
    pub fn new(
        telegram_id: TelegramId,
        uuid: Uuid,
        username: Option<String>,
        full_name: String,
        frozen_base_price: Money,
        referral_code: ReferralCode,
        subscription_token: SubscriptionToken,
    ) -> Self {
        Self {
            id: None,
            telegram_id,
            uuid,
            username,
            full_name,
            role: UserRole::User,
            frozen_base_price,
            referral_code,
            subscription_token,
            trial_used: false,
            discount_percent: DiscountPercent::zero(),
            created_at: Utc::now(),
        }
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn restore_from_db(
        id: UserId,
        telegram_id: TelegramId,
        uuid: Uuid,
        username: Option<String>,
        full_name: String,
        role: UserRole,
        frozen_base_price: Money,
        referral_code: ReferralCode,
        subscription_token: SubscriptionToken,
        trial_used: bool,
        discount_percent: DiscountPercent,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            telegram_id,
            uuid,
            username,
            full_name,
            role,
            frozen_base_price,
            referral_code,
            subscription_token,
            trial_used,
            discount_percent,
            created_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Option<UserId> {
        self.id
    }

    #[must_use]
    pub const fn telegram_id(&self) -> TelegramId {
        self.telegram_id
    }

    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[must_use]
    pub fn username(&self) -> Option<String> {
        self.username.clone()
    }

    #[must_use]
    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    #[must_use]
    pub const fn role(&self) -> UserRole {
        self.role
    }

    #[must_use]
    pub const fn frozen_base_price(&self) -> Money {
        self.frozen_base_price
    }

    #[must_use]
    pub const fn referral_code(&self) -> &ReferralCode {
        &self.referral_code
    }

    #[must_use]
    pub const fn subscription_token(&self) -> &SubscriptionToken {
        &self.subscription_token
    }

    #[must_use]
    pub const fn trial_used(&self) -> bool {
        self.trial_used
    }

    #[must_use]
    pub const fn discount_percent(&self) -> DiscountPercent {
        self.discount_percent
    }

    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.role == UserRole::Admin
    }

    pub const fn assign_id(&mut self, id: UserId) {
        self.id = Some(id);
    }

    pub fn use_trial(&mut self) -> DomainResult<()> {
        if self.trial_used {
            return Err(DomainError::User(UserError::TrialAlreadyUsed));
        }
        self.trial_used = true;
        self.discount_percent = DiscountPercent::new(15)?;
        Ok(())
    }

    pub fn update_profile(&mut self, username: Option<String>, full_name: String) {
        self.username = username;
        self.full_name = full_name;
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Days;

    // ==========================================
    // ВСПОМОГАТЕЛЬНАЯ ФУНКЦИЯ
    // ==========================================

    fn create_base_user() -> User {
        User::new(
            TelegramId::new(123456789),
            Uuid::new_v4(),
            Some("freddie".to_string()),
            "Freddie Mercury".to_string(),
            Money::new(20000).unwrap(),
            ReferralCode::new("MY_REF_123".to_string()),
            SubscriptionToken::new("TOKEN_XYZ".to_string()),
        )
    }

    // ==========================================
    // ТЕСТЫ
    // ==========================================

    #[test]
    fn test_user_new_creates_valid_default_user() {
        let user = create_base_user();
        assert_eq!(user.id, None);
        assert_eq!(user.telegram_id, TelegramId::new(123456789));
        assert_eq!(user.username, Some("freddie".to_string()));
        assert_eq!(user.full_name, "Freddie Mercury".to_string());
        assert_eq!(user.role, UserRole::User);
        assert_eq!(user.frozen_base_price, Money::new(20000).unwrap());
        assert_eq!(
            user.referral_code,
            ReferralCode::new("MY_REF_123".to_string())
        );
        assert_eq!(
            user.subscription_token,
            SubscriptionToken::new("TOKEN_XYZ".to_string())
        );
        assert_eq!(user.trial_used, false);
        assert_eq!(user.discount_percent, DiscountPercent::zero());
    }

    #[test]
    fn test_restore_from_db_and_getters() {
        let id = UserId::new(42);
        let tg_id = TelegramId::new(987654321);
        let uuid = Uuid::new_v4();
        let username = Some("ivan_ivanov".to_string());
        let full_name = "Ivan Ivanov".to_string();
        let role = UserRole::Admin;
        let base_price = Money::new(15000).unwrap();
        let ref_code = ReferralCode::new("PB5uCkAQB6j8".to_string());
        let sub_token = SubscriptionToken::new("4jw5emPkVq".to_string());
        let trial_used = true;
        let discount = DiscountPercent::new(10).unwrap();
        let created_at = Utc::now() - Days::new(5);

        let user = User::restore_from_db(
            id,
            tg_id,
            uuid,
            username.clone(),
            full_name.clone(),
            role,
            base_price,
            ref_code.clone(),
            sub_token.clone(),
            trial_used,
            discount,
            created_at,
        );

        // Погнали по геттерам!
        assert_eq!(user.id(), Some(id));
        assert_eq!(user.telegram_id(), tg_id);
        assert_eq!(user.uuid(), uuid);
        assert_eq!(user.username(), username);
        assert_eq!(user.full_name(), full_name);
        assert_eq!(user.role(), role);
        assert_eq!(user.frozen_base_price(), base_price);
        assert_eq!(user.referral_code(), &ref_code);
        assert_eq!(user.subscription_token(), &sub_token);
        assert_eq!(user.trial_used(), trial_used);
        assert_eq!(user.discount_percent(), discount);
        assert_eq!(user.created_at(), created_at);
    }

    #[test]
    fn test_assign_id_sets_correct_id() {
        let mut user = create_base_user();
        assert_eq!(
            user.id(),
            None,
            "У только что созданного юзера не должно быть ID"
        );

        let new_id = UserId::new(777);
        user.assign_id(new_id.clone());

        assert_eq!(
            user.id(),
            Some(new_id),
            "Метод assign_id должен корректно устанавливать ID"
        );
    }

    #[test]
    fn test_update_profile_changes_data_correctly() {
        let mut user = create_base_user();

        assert_eq!(user.username(), Some("freddie".to_string()));
        assert_eq!(user.full_name(), "Freddie Mercury");

        user.update_profile(None, "Farrokh Bulsara".to_string());

        assert_eq!(user.username(), None);
        assert_eq!(user.full_name(), "Farrokh Bulsara");

        user.update_profile(Some("queen_legend".to_string()), "Freddie Boss".to_string());

        assert_eq!(user.username(), Some("queen_legend".to_string()));
        assert_eq!(user.full_name(), "Freddie Boss");
    }

    #[test]
    fn test_use_trial_success() {
        let mut user = create_base_user();

        assert_eq!(user.discount_percent, DiscountPercent::zero());

        let result = user.use_trial();

        assert!(result.is_ok());
        assert_eq!(user.trial_used, true);
        assert_eq!(user.discount_percent, DiscountPercent::new(15).unwrap());
    }

    #[test]
    fn test_use_trial_fails_if_already_used() {
        let mut user = create_base_user();

        user.trial_used = true;

        let result = user.use_trial();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DomainError::User(UserError::TrialAlreadyUsed)
        ));
    }

    #[test]
    fn test_user_is_admin() {
        let mut user = create_base_user();
        assert_eq!(user.is_admin(), false);

        user.role = UserRole::Admin;
        assert_eq!(user.is_admin(), true);
    }
}
