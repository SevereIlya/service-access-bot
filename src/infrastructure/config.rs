use crate::application::error::AppResult;
use config::{Config, File};
use serde::Deserialize;
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub payments: PaymentsConfig,
    pub vpn: VpnConfig,
}

// ==============================================================================================
//                                          ПЛАТЕЖИ
// ==============================================================================================

#[derive(Clone, Debug, Deserialize)]
pub struct PaymentsConfig {
    pub base_price: i64,
    pub betatransfer: BetatransferConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BetatransferConfig {
    pub public_key: String,
    pub secret_key: String,
    pub ip_whitelist: Vec<IpAddr>,
    pub webhook_port: u16,
}

// ==============================================================================================
//                                   VPN, НОДЫ И ПРОТОКОЛЫ
// ==============================================================================================

#[derive(Clone, Debug, Deserialize)]
pub struct VpnConfig {
    pub nodes: Vec<VpnNodeConfig>,
    pub protocols: ProtocolsConfig,
    pub uuid_namespace: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VpnNodeConfig {
    pub name: String,
    pub ip: IpAddr,
    pub supported_protocols: Vec<String>,
    pub xui: Option<XuiConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProtocolsConfig {
    pub vless: Option<VlessConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VlessConfig {
    pub port: u16,
    pub public_key: String,
    pub sni: String,
    pub short_id: Vec<String>,
}

// ==============================================================================================
//                                       БАЗОВЫЕ ШТУКИ
// ==============================================================================================

#[derive(Clone, Debug, Deserialize)]
pub struct XuiConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub inbound_id: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GeneralConfig {
    pub database_url: String,
    pub publicbase_url: String,
    pub telegram_token: String,
    pub admin_chat_id: i64,
}

// ==============================================================================================

impl AppConfig {
    pub fn load() -> AppResult<Self> {
        let settings = Config::builder()
            .add_source(File::with_name("config.toml").required(true))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
