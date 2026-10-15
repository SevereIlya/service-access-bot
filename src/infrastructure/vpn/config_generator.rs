use crate::domain::{
    DomainError, DomainResult,
    vpn::{Node, VpnConfigGenerator},
};
use crate::infrastructure::AppConfig;
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

#[derive(Serialize)]
#[serde(tag = "type", rename = "socks5")]
struct StubProxy {
    name: String,
    server: String,
    port: u16,
}

pub struct ClashConfigGenerator {
    app_config: Arc<AppConfig>,
}

impl ClashConfigGenerator {
    #[must_use]
    pub const fn new(app_config: Arc<AppConfig>) -> Self {
        Self { app_config }
    }

    fn read_template(&self) -> DomainResult<String> {
        read_to_string(&self.app_config.vpn.template_path).map_err(|e| {
            error!(
                error = %e,
                path = %self.app_config.vpn.template_path,
                "Критическая ошибка: не удалось прочитать шаблон конфига с диска"
            );
            DomainError::SystemFailure("Шаблон VPN конфига не найден".into())
        })
    }
}

impl VpnConfigGenerator for ClashConfigGenerator {
    fn generate(&self, nodes: &[&Node], user_uuid: Uuid) -> DomainResult<String> {
        let template = self.read_template()?;

        let mut generated_proxies: Sequence = Vec::new();
        let mut proxy_names: Sequence = Vec::new();
        let user_key = user_uuid.to_string();

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
                    uuid: user_key.clone(),
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
                    password: user_key.clone(),
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
        render(&template, generated_proxies, proxy_names)
    }

    fn generate_stub(&self, messages: &[String]) -> DomainResult<String> {
        let template = self.read_template()?;
        let (proxies, names) = stub_parts(messages)?;
        render(&template, proxies, names)
    }
}

fn render(template: &str, proxies: Sequence, mut group_names: Sequence) -> DomainResult<String> {
    let mut config_tree: Value = serde_yaml::from_str(template).map_err(|e| {
        error!(error = %e, "Критическая ошибка: не удалось распарсить YAML шаблон");
        DomainError::SystemFailure("Ошибка парсинга YAML шаблона".into())
    })?;

    group_names.push(Value::String("DIRECT".to_string()));

    if let Some(map) = config_tree.as_mapping_mut() {
        map.insert(
            Value::String("proxies".to_string()),
            Value::Sequence(proxies),
        );

        if let Some(groups) = map
            .get_mut(Value::String("proxy-groups".to_string()))
            .and_then(|v| v.as_sequence_mut())
            && let Some(first_group) = groups.first_mut().and_then(|g| g.as_mapping_mut())
        {
            first_group.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(group_names),
            );
        }
    }
    serde_yaml::to_string(&config_tree)
        .map_err(|e| DomainError::SystemFailure(format!("Ошибка сборки YAML: {e}")))
}

fn stub_parts(messages: &[String]) -> DomainResult<(Sequence, Sequence)> {
    let mut proxies = Sequence::with_capacity(messages.len());
    let mut names = Sequence::with_capacity(messages.len());

    for message in messages {
        let proxy = StubProxy {
            name: message.clone(),
            server: "127.0.0.1".to_string(),
            port: 1,
        };

        let value = serde_yaml::to_value(proxy)
            .map_err(|e| DomainError::SystemFailure(format!("Ошибка сборки заглушки: {e}")))?;
        proxies.push(value);
        names.push(Value::String(message.clone()));
    }

    Ok((proxies, names))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const TEMPLATE: &str = "\
mode: rule
proxies: []
proxy-groups:
  - name: \"Proxy\"
    type: select
    proxies: []
rules:
  - MATCH,Proxy
";

    fn render_stub(messages: &[String]) -> Value {
        let (proxies, names) = stub_parts(messages).unwrap();
        let yaml = render(TEMPLATE, proxies, names).unwrap();
        serde_yaml::from_str(&yaml).expect("результат должен быть валидным YAML")
    }

    #[test]
    fn every_group_member_resolves_to_a_proxy_or_direct() {
        let messages = vec![
            "Впиши код в User Agent".to_owned(),
            "Обнови профиль".to_owned(),
        ];

        let tree = render_stub(&messages);

        let proxies = tree["proxies"].as_sequence().unwrap();

        assert_eq!(proxies.len(), 2);

        let known: HashSet<&str> = proxies.iter().filter_map(|p| p["name"].as_str()).collect();
        let group = tree["proxy-groups"][0]["proxies"].as_sequence().unwrap();
        for member in group {
            let name = member.as_str().unwrap();
            assert!(name == "DIRECT" || known.contains(name), "'{name}' нет среди proxies");
        }

        assert_eq!(group.last().and_then(Value::as_str), Some("DIRECT"));
        assert!(proxies.iter().all(|p| p["type"].as_str() == Some("socks5")));
    }

    #[test]
    fn template_sections_are_preserved() {
        let tree = render_stub(&["msg".to_owned()]);

        assert_eq!(tree["mode"].as_str(), Some("rule"));
        assert_eq!(tree["rules"][0].as_str(), Some("MATCH,Proxy"));
        assert_eq!(tree["proxy-groups"][0]["name"].as_str(), Some("Proxy"));
    }

    #[test]
    fn special_characters_in_messages_survive_yaml_roundtrip() {
        // Двоеточие, решётка, кавычки, эмодзи - опасные символы в YAML.
        let message = "⚠️ Код: abc # не комментарий, 'кавычки' \"двойные\"".to_owned();

        let tree = render_stub(std::slice::from_ref(&message));

        assert_eq!(tree["proxies"][0]["name"].as_str(), Some(message.as_str()));
        assert_eq!(tree["proxy-groups"][0]["proxies"][0].as_str(), Some(message.as_str()));
    }
}