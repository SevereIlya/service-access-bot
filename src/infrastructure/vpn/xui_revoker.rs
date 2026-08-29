use crate::domain::error::{DomainError, DomainResult};
use crate::domain::user::User;
use crate::domain::vpn::VpnAccessRevoker;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::vpn::XuiClient;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, instrument};

pub struct XuiAccessRevoker {
    clients: HashMap<String, XuiClient>,
}

impl XuiAccessRevoker {
    pub fn new(app_config: &Arc<AppConfig>) -> anyhow::Result<Self> {
        let mut clients = HashMap::new();
        for node in &app_config.vpn.nodes {
            let client = XuiClient::new(node.xui.base_url.clone(), &node.xui.api_token)?;
            clients.insert(node.name.clone(), client);
        }
        Ok(Self { clients })
    }
}

#[async_trait]
impl VpnAccessRevoker for XuiAccessRevoker {
    #[instrument(skip(self, user), fields(telegram_id = %user.telegram_id()))]
    async fn revoke_all(&self, user: &User) -> DomainResult<()> {
        let tg_id = user.telegram_id().inner();
        let mut has_errors = false;

        for (node_name, xui_client) in &self.clients {
            if let Err(e) = xui_client.disable_by_tgid(tg_id).await {
                error!(node = %node_name, error = %e, "Ошибка при отключении клиента в 3x-ui");
                has_errors = true;
            }
        }

        if has_errors {
            Err(DomainError::SystemFailure(
                "Не удалось отключить клиента на всех нодах".into(),
            ))
        } else {
            Ok(())
        }
    }
}
