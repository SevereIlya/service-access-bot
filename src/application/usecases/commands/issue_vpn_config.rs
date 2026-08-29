use crate::application::error::{AppError, AppResult};
use crate::domain::error::{
    DomainError, SubscriptionError::NoActiveSubscription, UserError::EntityNotSaved,
};
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::user::User;
use crate::domain::vpn::{
    DynNodeRepository, DynVpnConfigGenerator, DynVpnConnectionRepository, DynVpnProvisioner,
    VpnConnection,
};
use tracing::{error, warn};

pub struct IssueVpnConfigCommand {
    subscription_repo: DynSubscriptionRepository,
    node_repo: DynNodeRepository,
    vpn_connection_repo: DynVpnConnectionRepository,
    vpn_provisioner: DynVpnProvisioner,
    config_generator: DynVpnConfigGenerator,
}

impl IssueVpnConfigCommand {
    pub fn new(
        subscription_repo: DynSubscriptionRepository,
        node_repo: DynNodeRepository,
        vpn_connection_repo: DynVpnConnectionRepository,
        vpn_provisioner: DynVpnProvisioner,
        config_generator: DynVpnConfigGenerator,
    ) -> Self {
        Self {
            subscription_repo,
            node_repo,
            vpn_connection_repo,
            vpn_provisioner,
            config_generator,
        }
    }

    pub async fn execute(&self, user: &User) -> AppResult<String> {
        let user_id = user.id().ok_or(DomainError::User(EntityNotSaved))?;

        let subscription = self
            .subscription_repo
            .find_active_by_user_id(user_id)
            .await?
            .ok_or(DomainError::Subscription(NoActiveSubscription))?;
        let devices = subscription.devices();

        let nodes = self.node_repo.find_active_nodes().await?;
        if nodes.is_empty() {
            error!("В базе нет ни одной активной VPN-ноды");
            return Err(AppError::Internal("Нет доступных VPN-нод".into()));
        }

        let mut synced_nodes = Vec::with_capacity(nodes.len());
        for node in &nodes {
            if let Err(e) = self.vpn_provisioner.provision_node(node, user, devices).await {
                warn!(
                    node = node.name(),
                    user_id = %user_id,
                    error = %e,
                    "не удалось выдать доступ к ноде, пропускаем"
                );
                continue;
            }
            
            let node_id = node.id().ok_or_else(|| {
                error!(
                    node = node.name(),
                    user_id = %user_id,
                    "активная VPN-нода не имеет ID"
                );
                AppError::Internal("VPN-нода не имеет ID".into())
            })?;
            
            let mut connection = VpnConnection::new(user_id, node_id);
            connection.mark_as_synced();
            
            if let Err(e) = self.vpn_connection_repo.upsert(&connection).await {
                error!(
                    node = node.name(),
                    user_id = %user_id,
                    error = %e,
                    "доступ выдан, но не удалось записать это в БД"
                );
                continue;
            }
            
            synced_nodes.push(node);
        }

        if synced_nodes.is_empty() {
            error!(user_id = %user_id, "не удалось выдать доступ ни к одной ноде");
            return Err(AppError::Internal(
                "Не удалось подключиться ни к одной VPN-ноде".into(),
            ));
        }

        Ok(self.config_generator.generate(&synced_nodes, user.uuid()))
    }
}
