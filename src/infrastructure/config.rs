use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::fmt;
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
    pub platega: PlategaConfig,
}

#[derive(Clone, Deserialize)]
pub struct PlategaConfig {
    pub base_url: String,
    pub merchant_id: String,
    pub secret: String,
    pub success_url: String,
    pub failed_url: String,
    pub webhook_allowed_ip: Vec<IpAddr>,
}

// ==============================================================================================
//                                   VPN, НОДЫ И ПРОТОКОЛЫ
// ==============================================================================================

#[derive(Clone, Debug, Deserialize)]
pub struct VpnConfig {
    pub template_path: String,
    pub nodes: Vec<VpnNodeConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VpnNodeConfig {
    pub name: String,
    pub ip: IpAddr,
    pub xui: XuiConfig,
    pub vless: Option<VlessConfig>,
    pub hysteria2: Option<Hysteria2Config>,
}

#[derive(Clone, Deserialize)]
pub struct XuiConfig {
    pub base_url: String,
    pub api_token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VlessConfig {
    pub inbound_id: i32,
    pub port: u16,
    pub public_key: String,
    pub short_id: Vec<String>,
    pub sni: String,
    pub flow: String,
}

#[derive(Clone, Deserialize)]
pub struct Hysteria2Config {
    pub inbound_id: i32,
    pub port: u16,
    pub sni: String,
    pub obfs_password: String,
}

// ==============================================================================================
//                                       БАЗОВЫЕ ШТУКИ
// ==============================================================================================

#[derive(Clone, Deserialize)]
pub struct GeneralConfig {
    pub database_url: String,
    pub public_base_url: String,
    pub telegram_token: String,
    pub web_port: u16,
    pub admin_chat_id: i64,
    pub uuid_namespace: Uuid,
}

// ==============================================================================================
//                        Ручной Debug: секреты не утекают в логи и алерты
// ==============================================================================================

impl fmt::Debug for GeneralConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneralConfig")
            .field("database_url", &"[REDACTED]")
            .field("public_base_url", &self.public_base_url)
            .field("telegram_token", &"[REDACTED]")
            .field("web_port", &self.web_port)
            .field("admin_chat_id", &self.admin_chat_id)
            .field("uuid_namespace", &self.uuid_namespace)
            .finish()
    }
}

impl fmt::Debug for PlategaConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlategaConfig")
            .field("base_url", &self.base_url)
            .field("merchant_id", &self.merchant_id)
            .field("secret", &"[REDACTED]")
            .field("success_url", &self.success_url)
            .field("failed_url", &self.failed_url)
            .field("webhook_allowed_ip", &self.webhook_allowed_ip)
            .finish()
    }
}

impl fmt::Debug for XuiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XuiConfig")
            .field("base_url", &self.base_url)
            .field("api_token", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for Hysteria2Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hysteria2Config")
            .field("inbound_id", &self.inbound_id)
            .field("port", &self.port)
            .field("sni", &self.sni)
            .field("obfs_password", &"[REDACTED]")
            .finish()
    }
}

// ==============================================================================================

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(File::with_name("config.toml").required(true))
            .add_source(Environment::with_prefix("BOT").separator("__"))
            .build()?;
        settings.try_deserialize()
    }
}
