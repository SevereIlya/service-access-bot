use thiserror::Error;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("System failure: {0}")]
    SystemFailure(String),

    #[error(transparent)]
    User(#[from] UserError),

    #[error(transparent)]
    Subscription(#[from] SubscriptionError),
}

#[derive(Error, Debug)]
pub enum UserError {
    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,
    #[error("User not found")]
    NotFound,
    #[error("User already exists")]
    AlreadyExists,
    #[error("Referral code collision")]
    ReferralCodeCollision,
    #[error("Trial already used")]
    TrialAlreadyUsed,

    #[error("Invalid role: {0}")]
    InvalidRole(String),
    #[error("Invalid discount: {0}")]
    InvalidDiscount(i32),
    #[error("Invalid money: {0}")]
    InvalidMoney(i64),
}

#[derive(Error, Debug)]
pub enum SubscriptionError  {
    #[error("Already has active subscription")]
    AlreadyHasActive,
    #[error("Invalid subscription plan: {0}")]
    InvalidPlan(String),
    #[error("Invalid subscription status: {0}")]
    InvalidStatus(String),
    #[error("Invalid devices count: {0}")]
    InvalidDevices(i32),
}