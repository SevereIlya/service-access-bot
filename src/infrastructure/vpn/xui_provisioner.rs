use crate::domain::error::{DomainError, DomainResult};
use crate::domain::subscription::SubscriptionDevices;
use crate::domain::user::User;
use crate::domain::vpn::{Node, VpnProvisioner};
use crate::infrastructure::config::{AppConfig, VpnNodeConfig};
use crate::infrastructure::vpn::{ClientObj, XuiClient};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, instrument};

pub struct XuiProvisioner {
    clients: HashMap<String, (VpnNodeConfig, XuiClient)>,
}

impl XuiProvisioner {
    pub fn new(app_config: &Arc<AppConfig>) -> anyhow::Result<Self> {
        let mut clients = HashMap::new();
        for node in &app_config.vpn.nodes {
            let client = XuiClient::new(node.xui.base_url.clone(), &node.xui.api_token)?;
            clients.insert(node.name.clone(), (node.clone(), client));
        }
        Ok(Self { clients })
    }
}

#[async_trait]
impl VpnProvisioner for XuiProvisioner {
    #[instrument(skip(self, user), fields(telegram_id = %user.telegram_id(), node = %node.name()))]
    async fn provision_node(
        &self,
        node: &Node,
        user: &User,
        devices: SubscriptionDevices,
    ) -> DomainResult<()> {
        let Some((node_cfg, xui_client)) = self.clients.get(node.name()) else {
            return Err(DomainError::SystemFailure(format!(
                "Нода {} есть в БД, но не найдена в config.toml",
                node.name()
            )));
        };

        let mut inbound_ids = Vec::new();
        let flow = node_cfg.vless.as_ref().map_or_else(String::new, |vless| {
            inbound_ids.push(vless.inbound_id);
            vless.flow.clone()
        });

        if let Some(hys2) = &node_cfg.hysteria2 {
            inbound_ids.push(hys2.inbound_id);
        }

        if inbound_ids.is_empty() {
            return Err(DomainError::SystemFailure(format!(
                "На ноде {} не настроен ни один протокол из config.toml",
                node.name()
            )));
        }

        let client_id = user.uuid().to_string();
        let email = user.username().map_or_else(
            || format!("user_{}", user.telegram_id()),
            |username| format!("{username}_{}", user.telegram_id()),
        );

        let client = ClientObj {
            id: client_id.clone(),
            email: email.clone(),
            sub_id: user.subscription_token().inner().to_string(),
            password: client_id.clone(),
            auth: client_id.clone(),
            flow,
            limit_ip: 0,
            limit_hwid: devices.inner(),
            total_gb: 0,
            expiry_time: 0,
            enable: true,
            tg_id: user.telegram_id().inner(),
            comment: user.full_name().to_string(),
        };

        if let Err(e) = xui_client.upsert(inbound_ids, &email, &client).await {
            error!(error = %e, "Не удалось добавить/обновить клиента в 3x-ui");
            return Err(DomainError::SystemFailure(e.to_string()));
        }

        Ok(())
    }
}
