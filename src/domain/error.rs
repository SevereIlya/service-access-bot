use thiserror::Error;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("System failure: {0}")]
    SystemFailure(String),

    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,
    #[error("User not found")]
    UserNotFound,
    #[error("User already exists")]
    UserAlreadyExists,
    #[error("Referral code collision")]
    ReferralCodeCollision,
    #[error("Invalid role: {0}")]
    InvalidRole(String),
    #[error("Invalid discount: {0}")]
    InvalidDiscount(i32),
    #[error("Trial already used")]
    TrialAlreadyUsed,

    #[error("Invalid subscription plan: {0}")]
    InvalidPlan(String),
    #[error("Invalid subscription status: {0}")]
    InvalidStatus(String),
    #[error("Already has active subscription")]
    AlreadyHasSubscription,
}