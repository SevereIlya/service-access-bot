use crate::application::error::AppResult;
use crate::domain::user::{DynUserRepository, TelegramId, User};

pub struct GetUserQuery {
    user_repo: DynUserRepository,
}

impl GetUserQuery {
    pub fn new(user_repo: DynUserRepository) -> Self {
        Self { user_repo }
    }

    pub async fn execute(&self, telegram_id: i64) -> AppResult<Option<User>> {
        let user =
            self.user_repo.find_by_telegram_id(TelegramId::new(telegram_id)).await?;
        Ok(user)
    }
}

// ==============================================================================================
//                                          ТЕСТЫ
// ==============================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::domain::error::DomainResult;
    use crate::domain::user::{
        Money, ReferralCode, SubscriptionToken, TelegramId, UserId, UserRepository,
    };

    // ==========================================
    // Моки
    // ==========================================

    struct MockUserRepository {
        user_to_return: Option<User>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _user: &User) -> DomainResult<UserId> {
            unreachable!("create() не используется в этом юзкейсе")
        }

        async fn update(&self, _user: &User) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }

        async fn find_by_user_id(&self, _id: UserId) -> DomainResult<Option<User>> {
            unreachable!("find_by_user_id() не используется в этом юзкейсе")
        }

        async fn find_by_telegram_id(
            &self,
            _telegram_id: TelegramId,
        ) -> DomainResult<Option<User>> {
            Ok(self.user_to_return.clone())
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn create_test_user(tg_id: i64) -> User {
        let mut user = User::new(
            TelegramId::new(tg_id),
            Uuid::new_v4(),
            None,
            "Freddie".to_string(),
            Money::new(1000).unwrap(),
            ReferralCode::new("REF123".to_string()),
            SubscriptionToken::new("TOKEN".to_string()),
        );
        user.assign_id(UserId::new(1));
        user
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_get_user_returns_user_when_found() {
        let target_tg_id = 999999;
        let existing_user = create_test_user(target_tg_id);

        let repo = Arc::new(MockUserRepository {
            user_to_return: Some(existing_user),
        });

        let query = GetUserQuery::new(repo);
        let result = query.execute(target_tg_id).await.unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().telegram_id().inner(), target_tg_id);
    }

    #[tokio::test]
    async fn test_get_user_returns_none_when_not_found() {
        let repo = Arc::new(MockUserRepository {
            user_to_return: None,
        });

        let query = GetUserQuery::new(repo);
        let result = query.execute(12345).await.unwrap();

        assert!(result.is_none());
    }
}
