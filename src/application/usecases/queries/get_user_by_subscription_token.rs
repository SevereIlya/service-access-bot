use crate::application::AppResult;
use crate::domain::user::{DynUserRepository, SubscriptionToken, User};

pub struct GetUserBySubscriptionTokenQuery {
    user_repo: DynUserRepository,
}

impl GetUserBySubscriptionTokenQuery {
    pub fn new(user_repo: DynUserRepository) -> Self {
        Self { user_repo }
    }

    pub async fn execute(&self, token: &str) -> AppResult<Option<User>> {
        let token = SubscriptionToken::new(token.to_string());
        let user = self.user_repo.find_by_subscription_token(&token).await?;
        Ok(user)
    }
}
