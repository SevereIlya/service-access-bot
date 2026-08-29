use crate::domain::error::DomainError;
use crate::domain::subscription::{Subscription, SubscriptionDevices, SubscriptionId};
use crate::domain::user::{
    DiscountPercent, Money, ReferralCode, SubscriptionToken, TelegramId, User, UserId,
};
use crate::domain::vpn::{Node, NodeId, VpnConnection, VpnConnectionId};
use crate::infrastructure::database::{NodeRow, SubscriptionRow, UserRow, VpnConnectionRow};

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