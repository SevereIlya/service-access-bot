use crate::application::error::AppResult;
use crate::domain::error::DomainError;
use crate::domain::error::UserError::EntityNotSaved;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::user::User;

pub struct MenuState {
    pub can_trial: bool,
}

pub struct GetMenuStateQuery {
    sub_repo: DynSubscriptionRepository,
}

impl GetMenuStateQuery {
    pub fn new(sub_repo: DynSubscriptionRepository) -> Self {
        Self { sub_repo }
    }

    pub async fn execute(&self, user: &User) -> AppResult<MenuState> {
        if user.trial_used() {
            return Ok(MenuState { can_trial: false });
        }

        let id = user.id().ok_or(DomainError::User(EntityNotSaved))?;
        let has_active_sub = self.sub_repo.find_active_by_user_id(id).await?.is_some();

        Ok(MenuState {
            can_trial: !has_active_sub,
        })
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{DateTime, Days, Utc};
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::domain::error::DomainResult;
    use crate::domain::subscription::{
        Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionRepository,
        SubscriptionStatus,
    };
    use crate::domain::user::{
        Money, ReferralCode, SubscriptionToken, TelegramId, UserId,
    };

    // ==========================================
    // Моки
    // ==========================================

    struct MockSubscriptionRepository {
        active_sub_to_return: Option<Subscription>,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("create() не используется в этом юзкейсе")
        }

        async fn update(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }

        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            Ok(self.active_sub_to_return.clone())
        }

        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_lapsed_active() не используется в этом юзкейсе")
        }

        async fn find_expiring_between(
            &self,
            _start: DateTime<Utc>,
            _end: DateTime<Utc>,
        ) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_expiring_between() не используется в этом юзкейсе")
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn create_test_user(use_trial: bool, id: Option<UserId>) -> User {
        let mut user = User::new(
            TelegramId::new(123456789),
            Uuid::new_v4(),
            None,
            "John Doe".to_string(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF123".to_string()),
            SubscriptionToken::new("TOKEN".to_string()),
        );

        if use_trial {
            let _ = user.use_trial();
        }

        if let Some(user_id) = id {
            user.assign_id(user_id);
        }

        user
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_menu_state_can_trial_is_false_if_trial_already_used() {
        let user = create_test_user(true, Some(UserId::new(1)));
        let repo = Arc::new(MockSubscriptionRepository {
            active_sub_to_return: None,
        });

        let query = GetMenuStateQuery::new(repo);
        let state = query.execute(&user).await.unwrap();

        assert!(
            !state.can_trial,
            "Trial нельзя взять, если он уже использован"
        );
    }

    #[tokio::test]
    async fn test_menu_state_returns_error_if_user_has_no_id() {
        let user = create_test_user(false, None);
        let repo = Arc::new(MockSubscriptionRepository {
            active_sub_to_return: None,
        });

        let query = GetMenuStateQuery::new(repo);
        let result = query.execute(&user).await;

        assert!(
            result.is_err(),
            "Должна быть ошибка, если у пользователя нет ID"
        );
    }

    #[tokio::test]
    async fn test_menu_state_can_trial_is_false_if_has_active_sub() {
        let user = create_test_user(false, Some(UserId::new(1)));

        let active_sub = Subscription::new(
            UserId::new(1),
            SubscriptionPlan::Month1,
            Utc::now(),
            Utc::now() + Days::new(30),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(1).unwrap(),
        );

        let repo = Arc::new(MockSubscriptionRepository {
            active_sub_to_return: Some(active_sub),
        });

        let query = GetMenuStateQuery::new(repo);
        let state = query.execute(&user).await.unwrap();

        assert!(
            !state.can_trial,
            "Trial нельзя взять, если есть активная подписка"
        );
    }

    #[tokio::test]
    async fn test_menu_state_can_trial_is_true_if_no_active_sub_and_trial_not_used() {
        let user = create_test_user(false, Some(UserId::new(1)));

        let repo = Arc::new(MockSubscriptionRepository {
            active_sub_to_return: None,
        });

        let query = GetMenuStateQuery::new(repo);
        let state = query.execute(&user).await.unwrap();

        assert!(
            state.can_trial,
            "Trial можно взять, если нет подписок и он не юзался"
        );
    }
}
