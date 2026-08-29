use crate::domain::vpn::{Node, VpnConfigGenerator};
use crate::infrastructure::config::AppConfig;
use serde::Serialize;
use serde_yaml::{Sequence, Value};
use std::fs::read_to_string;
use std::sync::Arc;
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Serialize)]
#[serde(tag = "type", rename = "vless")]
struct VlessProxy {
    name: String,
    server: String,
    port: u16,
    uuid: String,
    network: String,
    tls: bool,
    udp: bool,
    flow: String,
    servername: String,
    #[serde(rename = "client-fingerprint")]
    client_fingerprint: String,
    #[serde(rename = "packet-encoding")]
    packet_encoding: String,
    #[serde(rename = "reality-opts")]
    reality_opts: RealityOpts,
}

#[derive(Serialize)]
struct RealityOpts {
    #[serde(rename = "public-key")]
    public_key: String,
    #[serde(rename = "short-id")]
    short_id: String,
    #[serde(rename = "spider-x")]
    spider_x: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "hysteria2")]
struct Hysteria2Proxy {
    name: String,
    server: String,
    port: u16,
    password: String,
    sni: String,
    #[serde(rename = "skip-cert-verify")]
    skip_cert_verify: bool,
    alpn: Vec<String>,
    obfs: String,
    #[serde(rename = "obfs-password")]
    obfs_password: String,
}

pub struct ClashConfigGenerator {
    app_config: Arc<AppConfig>,
}

impl ClashConfigGenerator {
    #[must_use]
    pub const fn new(app_config: Arc<AppConfig>) -> Self {
        Self { app_config }
    }
}

impl VpnConfigGenerator for ClashConfigGenerator {
    fn generate(&self, nodes: &[&Node], user_uuid: Uuid) -> String {
        let template_str = match read_to_string(&self.app_config.vpn.template_path) {
            Ok(content) => content,
            Err(e) => {
                error!(error = %e, path = %self.app_config.vpn.template_path, "Не удалось прочитать шаблон конфига");
                return "Ошибка генерации конфига: шаблон не найден".to_string();
            }
        };

        let mut config_tree: Value = match serde_yaml::from_str(&template_str) {
            Ok(val) => val,
            Err(e) => {
                error!(error = %e, "Критическая ошибка: не удалось распарсить YAML шаблон");
                return "Ошибка генерации конфига: сломан синтаксис YAML в шаблоне.".to_string();
            }
        };

        let mut generated_proxies: Sequence = Vec::new();
        let mut proxy_names: Sequence = Vec::new();
        let client_id_str = user_uuid.to_string();

        for node in nodes {
            let Some(node_cfg) = self.app_config.vpn.nodes.iter().find(|n| n.name == node.name())
            else {
                warn!(
                    node = node.name(),
                    "Нода есть в БД, но отсутствует в config.toml"
                );
                continue;
            };

            if let Some(vless) = &node_cfg.vless {
                let name = format!("{} (Vless)", node.name());
                let proxy = VlessProxy {
                    name: name.clone(),
                    server: node.ip_address().to_string(),
                    port: vless.port,
                    uuid: client_id_str.clone(),
                    network: "tcp".to_string(),
                    tls: true,
                    udp: true,
                    flow: vless.flow.clone(),
                    servername: vless.sni.clone(),
                    client_fingerprint: "firefox".to_string(),
                    packet_encoding: "xudp".to_string(),
                    reality_opts: RealityOpts {
                        public_key: vless.public_key.clone(),
                        short_id: vless.short_id.first().cloned().unwrap_or_default(),
                        spider_x: "/media".to_string(),
                    },
                };

                if let Ok(val) = serde_yaml::to_value(proxy) {
                    generated_proxies.push(val);
                    proxy_names.push(Value::String(name));
                }
            }

            if let Some(hy2) = &node_cfg.hysteria2 {
                let name = format!("{} (Hysteria 2)", node.name());
                let proxy = Hysteria2Proxy {
                    name: name.clone(),
                    server: node.ip_address().to_string(),
                    port: hy2.port,
                    password: client_id_str.clone(),
                    sni: hy2.sni.clone(),
                    skip_cert_verify: true,
                    alpn: vec!["h3".to_string()],
                    obfs: "salamander".to_string(),
                    obfs_password: hy2.obfs_password.clone(),
                };

                if let Ok(val) = serde_yaml::to_value(proxy) {
                    generated_proxies.push(val);
                    proxy_names.push(Value::String(name));
                }
            }
        }

        proxy_names.push(Value::String("DIRECT".to_string()));

        if let Some(map) = config_tree.as_mapping_mut() {
            map.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(generated_proxies),
            );

            if let Some(groups) = map
                .get_mut(Value::String("proxy-groups".to_string()))
                .and_then(|v| v.as_sequence_mut())
                && let Some(first_group) = groups.first_mut().and_then(|g| g.as_mapping_mut())
            {
                first_group.insert(
                    Value::String("proxies".to_string()),
                    Value::Sequence(proxy_names),
                );
            }
        }

        serde_yaml::to_string(&config_tree).unwrap_or_else(|_| "Ошибка сборки YAML".to_string())
    }
}
