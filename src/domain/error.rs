use thiserror::Error;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error(transparent)]
    Subscription(#[from] SubscriptionError),

    #[error("System failure: {0}")]
    SystemFailure(String),

    #[error(transparent)]
    User(#[from] UserError),
}

#[derive(Error, Debug)]
pub enum UserError {
    #[error("User already exists")]
    AlreadyExists,

    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,

    #[error("Invalid discount: {0}")]
    InvalidDiscount(i32),

    #[error("Invalid money: {0}")]
    InvalidMoney(i64),

    #[error("Invalid role: {0}")]
    InvalidRole(String),

    #[error("Referral code collision")]
    ReferralCodeCollision,

    #[error("Trial already used")]
    TrialAlreadyUsed,

    #[error("User not found")]
    NotFound,
}

#[derive(Error, Debug)]
pub enum SubscriptionError {
    #[error("Already has active subscription")]
    AlreadyHasActive,

    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,

    #[error("Invalid devices count: {0}")]
    InvalidDevices(i32),

    #[error("Invalid subscription plan: {0}")]
    InvalidPlan(String),

    #[error("Invalid subscription status: {0}")]
    InvalidStatus(String),
}