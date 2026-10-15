use crate::domain::billing::TransactionId;
use crate::domain::{
    DomainError,
    billing::{Money, Transaction},
    device::{Device, DeviceFingerprint, DeviceId, DeviceName, DeviceSignal},
    subscription::{Subscription, SubscriptionDevices, SubscriptionId},
    user::{DiscountPercent, ReferralCode, SubscriptionToken, TelegramId, User, UserId},
    vpn::{Node, NodeId, VpnConnection, VpnConnectionId},
};
use crate::infrastructure::{
    DeviceRow, NodeRow, SubscriptionRow, TransactionRow, UserRow, VpnConnectionRow,
};

impl TryFrom<UserRow> for User {
    type Error = DomainError;

    fn try_from(row: UserRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            UserId::new(row.id),
            TelegramId::new(row.telegram_id),
            row.uuid,
            row.username,
            row.full_name,
            row.role.parse()?,
            Money::new(row.frozen_base_price)?,
            ReferralCode::new(row.referral_code),
            SubscriptionToken::new(row.subscription_token),
            row.trial_used,
            DiscountPercent::new(row.discount_percent)?,
            row.created_at,
        ))
    }
}

impl TryFrom<SubscriptionRow> for Subscription {
    type Error = DomainError;

    fn try_from(row: SubscriptionRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            SubscriptionId::new(row.id),
            UserId::new(row.user_id),
            row.plan.parse()?,
            row.starts_at,
            row.expires_at,
            row.status.parse()?,
            SubscriptionDevices::new(row.devices)?,
            row.is_warning_sent,
            row.created_at,
        ))
    }
}

impl TryFrom<DeviceRow> for Device {
    type Error = DomainError;

    fn try_from(row: DeviceRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            DeviceId::new(row.id),
            UserId::new(row.user_id),
            DeviceFingerprint::new(row.fingerprint),
            DeviceName::restore(row.name),
            DeviceSignal::new(
                row.hardware_id,
                row.os_name,
                row.os_version,
                row.device_model,
                row.user_agent,
            ),
            row.status.parse()?,
            row.first_seen_at,
            row.last_seen_at,
        ))
    }
}

impl TryFrom<NodeRow> for Node {
    type Error = DomainError;

    fn try_from(row: NodeRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            NodeId::new(row.id),
            row.name,
            row.ip_address.parse()?,
            row.is_active,
            row.created_at,
        ))
    }
}

impl TryFrom<VpnConnectionRow> for VpnConnection {
    type Error = DomainError;

    fn try_from(row: VpnConnectionRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            VpnConnectionId::new(row.id),
            UserId::new(row.user_id),
            NodeId::new(row.node_id),
            row.is_synced,
            row.created_at,
        ))
    }
}

impl TryFrom<TransactionRow> for Transaction {
    type Error = DomainError;

    fn try_from(row: TransactionRow) -> Result<Self, Self::Error> {
        Ok(Self::restore_from_db(
            TransactionId::new(row.id),
            UserId::new(row.user_id),
            Money::new(row.amount)?,
            row.kind.parse()?,
            row.status.parse()?,
            row.payment_provider.parse()?,
            row.order_id,
            row.provider_tx_id,
            row.payload,
            row.created_at,
        ))
    }
}
