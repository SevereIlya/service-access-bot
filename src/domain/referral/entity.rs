use crate::domain::{DomainResult, ReferralError, referral::ReferralId, user::UserId};
use chrono::{DateTime, Utc};

/// `referrer` пригласил `referred`.
///
/// Запись создаётся один раз и больше не обновляется.
#[derive(Debug, Clone)]
pub struct Referral {
    id: Option<ReferralId>,
    referrer_id: UserId,
    referred_id: UserId,
    created_at: DateTime<Utc>,
}

impl Referral {
    #[allow(clippy::similar_names)]
    pub fn new(referrer_id: UserId, referred_id: UserId) -> DomainResult<Self> {
        if referrer_id == referred_id {
            return Err(ReferralError::SelfReferral.into());
        }

        Ok(Self {
            id: None,
            referrer_id,
            referred_id,
            created_at: Utc::now(),
        })
    }

    #[must_use]
    #[allow(clippy::similar_names)]
    pub const fn restore_from_db(
        id: ReferralId,
        referrer_id: UserId,
        referred_id: UserId,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Some(id),
            referrer_id,
            referred_id,
            created_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Option<ReferralId> {
        self.id
    }

    #[must_use]
    pub const fn referrer_id(&self) -> UserId {
        self.referrer_id
    }

    #[must_use]
    pub const fn referred_id(&self) -> UserId {
        self.referred_id
    }

    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub const fn assign_id(&mut self, id: ReferralId) {
        self.id = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainError;

    #[test]
    fn test_referral_new_links_two_different_users() {
        let referral = Referral::new(UserId::new(1), UserId::new(2)).unwrap();

        assert_eq!(referral.id(), None, "id выдаёт БД");
        assert_eq!(referral.referrer_id(), UserId::new(1));
        assert_eq!(referral.referred_id(), UserId::new(2));
    }

    #[test]
    fn test_referral_new_rejects_self_referral() {
        let err = Referral::new(UserId::new(7), UserId::new(7)).unwrap_err();

        assert!(matches!(
            err,
            DomainError::Referral(ReferralError::SelfReferral)
        ));
    }

    #[test]
    fn test_referral_assign_id() {
        let mut referral = Referral::new(UserId::new(1), UserId::new(2)).unwrap();

        referral.assign_id(ReferralId::new(5));

        assert_eq!(referral.id(), Some(ReferralId::new(5)));
    }

    #[test]
    fn test_referral_restore_from_db_keeps_every_field() {
        let created_at = Utc::now();

        let referral = Referral::restore_from_db(
            ReferralId::new(9),
            UserId::new(1),
            UserId::new(2),
            created_at,
        );

        assert_eq!(referral.id(), Some(ReferralId::new(9)));
        assert_eq!(referral.referrer_id(), UserId::new(1));
        assert_eq!(referral.referred_id(), UserId::new(2));
        assert_eq!(referral.created_at(), created_at);
    }
}
