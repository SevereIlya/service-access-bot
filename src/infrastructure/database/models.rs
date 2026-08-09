use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: i64,
    pub telegram_id: i64,
    pub uuid: Uuid,
    pub username: Option<String>,
    pub full_name: String,
    pub role: String,
    pub frozen_base_price: i64,
    pub referral_code: String,
    pub subscription_token: String,
    pub trial_used: bool,
    pub discount_percent: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct SubscriptionRow {
    pub id: i64,
    pub user_id: i64,
    pub plan: String,
    pub starts_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: String,
    pub devices: i32,
    pub created_at: DateTime<Utc>,
}