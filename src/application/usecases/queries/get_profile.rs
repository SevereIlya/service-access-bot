use crate::application::AppResult;
use crate::domain::{
    DomainError, UserError,
    subscription::{DynSubscriptionRepository, SubscriptionPlan},
    user::User,
};
use chrono::{DateTime, Utc};

pub struct ProfileSnapshot {
    pub telegram_id: i64,
    pub referral_code: String,
    pub subscription: Option<ProfileSubscription>,
    // TODO(billing): balance_main / balance_referral — когда ledger встанет на Platega, тут появится вызов transaction_repo.balance_for_user().
}

pub struct ProfileSubscription {
    pub plan: SubscriptionPlan,
    pub expires_at: DateTime<Utc>,
    pub days_left: i64,
}

pub struct GetProfileQuery {
    subscription_repo: DynSubscriptionRepository,
}

impl GetProfileQuery {
    pub fn new(subscription_repo: DynSubscriptionRepository) -> Self {
        Self { subscription_repo }
    }

    pub async fn execute(&self, user: &User) -> AppResult<ProfileSnapshot> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;
        let active_sub = self.subscription_repo.find_active_by_user_id(user_id).await?;
        let subscription = active_sub.map(|sub| ProfileSubscription {
            plan: sub.plan(),
            expires_at: sub.expires_at(),
            days_left: (sub.expires_at() - Utc::now()).num_days(),
        });
        Ok(ProfileSnapshot {
            telegram_id: user.telegram_id().inner(),
            referral_code: user.referral_code().inner().to_string(),
            subscription,
        })
    }
}
