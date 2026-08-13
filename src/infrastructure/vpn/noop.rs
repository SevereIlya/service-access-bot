use crate::domain::error::DomainResult;
use crate::domain::user::UserId;
use crate::domain::vpn::VpnAccessRevoker;
use async_trait::async_trait;
use tracing::warn;


/// Временная заглушка
pub struct NoopVpnAccessRevoker;

#[async_trait]
impl VpnAccessRevoker for NoopVpnAccessRevoker {
    async fn revoke_all(&self, user_id: UserId) -> DomainResult<()> {
        warn!(
            user_id = %user_id,
            "VPN ещё не реализован. revoke_all() - no-op"
        );
        Ok(())
    }
}