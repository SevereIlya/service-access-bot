use crate::domain::{billing::Money, device::DeviceStatus};
use thiserror::Error;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error(transparent)]
    Billing(#[from] BillingError),

    #[error(transparent)]
    Device(#[from] DeviceError),

    #[error(transparent)]
    Node(#[from] NodeError),

    #[error(transparent)]
    Subscription(#[from] SubscriptionError),

    #[error("System failure: {0}")]
    SystemFailure(String),

    #[error(transparent)]
    User(#[from] UserError),
}

#[derive(Error, Debug)]
pub enum BillingError {
    #[error("Amount out of range: {0}")]
    AmountOutOfRange(f64),

    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: Money, available: Money },

    #[error("Invalid amount: {0}")]
    InvalidAmount(i64),

    #[error("Invalid payment provider: {0}")]
    InvalidPaymentProvider(String),

    #[error("Invalid transaction status: {0}")]
    InvalidTransactionStatus(String),

    #[error("Invalid transaction kind: {0}")]
    InvalidTransactionKind(String),

    #[error("Non finite amount")]
    NonFiniteAmount,

    #[error("Subscription plan is not purchasable: {0}")]
    PlanNotPurchasable(String),
}

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Entity has no ID. It must be saved to the database first.")]
    EntityNotSaved,

    #[error("Invalid device status: {0}")]
    InvalidStatus(String),

    #[error("Invalid device status transition: {from} -> {to}")]
    InvalidStatusTransition {
        from: DeviceStatus,
        to: DeviceStatus,
    },
}

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Invalid IP address: {0}")]
    InvalidIpAddress(String),
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

    #[error("No active subscription")]
    NoActiveSubscription,
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
