use crate::application::AppResult;
use crate::domain::{
    DomainError, UserError,
    billing::{DynTransactionRepository, Money},
    device::DynDeviceRepository,
    subscription::{DynSubscriptionRepository, SubscriptionPlan},
    user::User,
};
use chrono::{DateTime, Utc};

pub struct ProfileSnapshot {
    pub telegram_id: i64,
    pub balance: Money,
    pub subscription: Option<ProfileSubscription>,
}

pub struct ProfileSubscription {
    pub plan: SubscriptionPlan,
    pub expires_at: DateTime<Utc>,
    pub days_left: i64,
    pub devices: ProfileDevices,
}

pub struct ProfileDevices {
    pub devices_limit: i32,
    pub devices_used: usize,
}

#[allow(clippy::struct_field_names)]
pub struct GetProfileQuery {
    subscription_repo: DynSubscriptionRepository,
    transaction_repo: DynTransactionRepository,
    device_repo: DynDeviceRepository,
}

impl GetProfileQuery {
    pub fn new(
        subscription_repo: DynSubscriptionRepository,
        transaction_repo: DynTransactionRepository,
        device_repo: DynDeviceRepository,
    ) -> Self {
        Self {
            subscription_repo,
            transaction_repo,
            device_repo,
        }
    }

    pub async fn execute(&self, user: &User) -> AppResult<ProfileSnapshot> {
        let user_id = user.id().ok_or(DomainError::User(UserError::EntityNotSaved))?;

        let (active_sub, balance, devices) = tokio::try_join!(
            self.subscription_repo.find_active_by_user_id(user_id),
            self.transaction_repo.balance_for_user(user_id),
            self.device_repo.find_all_by_user(user_id),
        )?;

        let devices_used =
            devices.iter().filter(|device| device.status().counts_toward_limit()).count();

        let subscription = active_sub.map(|sub| ProfileSubscription {
            plan: sub.plan(),
            expires_at: sub.expires_at(),
            days_left: (sub.expires_at() - Utc::now()).num_days(),
            devices: ProfileDevices {
                devices_limit: sub.devices().inner(),
                devices_used,
            },
        });
        Ok(ProfileSnapshot {
            telegram_id: user.telegram_id().inner(),
            balance,
            subscription,
        })
    }
}
