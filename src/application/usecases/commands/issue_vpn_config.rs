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

            let Some(node_id) = node.id() else {
                warn!(
                    node = node.name(),
                    user_id = %user_id,
                    "БД вернула ноду без ID, пропускаем сохранение стейта"
                );
                continue;
            };

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

        let config_str = self.config_generator.generate(&synced_nodes, user.uuid())?;
        Ok(config_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::DomainResult;
    use crate::domain::subscription::{
        Subscription, SubscriptionDevices, SubscriptionPlan, SubscriptionRepository,
        SubscriptionStatus,
    };
    use crate::domain::user::{Money, ReferralCode, SubscriptionToken, TelegramId, UserId};
    use crate::domain::vpn::{
        Node, NodeId, NodeIpAddress, NodeRepository, VpnConfigGenerator, VpnConnectionRepository,
        VpnProvisioner,
    };
    use async_trait::async_trait;
    use chrono::{Days, Utc};
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // ==========================================
    // Моки
    // ==========================================

    struct MockSubscriptionRepository {
        active: Option<Subscription>,
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn create(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("create() не используется в этом юзкейсе")
        }
        async fn update(&self, _subscription: &Subscription) -> DomainResult<()> {
            unreachable!("update() не используется в этом юзкейсе")
        }
        async fn find_active_by_user_id(
            &self,
            _user_id: UserId,
        ) -> DomainResult<Option<Subscription>> {
            Ok(self.active.clone())
        }
        async fn find_lapsed_active(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_lapsed_active() не используется в этом юзкейсе")
        }
        async fn find_due_for_expiry_warning(&self) -> DomainResult<Vec<Subscription>> {
            unreachable!("find_due_for_expiry_warning() не используется в этом юзкейсе")
        }
    }

    struct MockNodeRepository {
        nodes: Vec<Node>,
    }

    #[async_trait]
    impl NodeRepository for MockNodeRepository {
        async fn find_active_nodes(&self) -> DomainResult<Vec<Node>> {
            Ok(self.nodes.clone())
        }
    }

    struct MockVpnConnectionRepository {
        fail_for: Vec<NodeId>,
        upserted: Arc<Mutex<Vec<VpnConnection>>>,
    }

    #[async_trait]
    impl VpnConnectionRepository for MockVpnConnectionRepository {
        async fn find_by_user_and_node(
            &self,
            _user_id: UserId,
            _node_id: NodeId,
        ) -> DomainResult<Option<VpnConnection>> {
            unreachable!("find_by_user_and_node() не используется в этом юзкейсе")
        }
        async fn upsert(&self, connection: &VpnConnection) -> DomainResult<()> {
            if self.fail_for.contains(&connection.node_id()) {
                return Err(DomainError::SystemFailure("upsert failed (test)".into()));
            }
            self.upserted.lock().unwrap().push(connection.clone());
            Ok(())
        }
    }

    struct MockVpnProvisioner {
        fail_for: Vec<String>,
        calls: Arc<Mutex<Vec<(String, SubscriptionDevices)>>>,
    }

    #[async_trait]
    impl VpnProvisioner for MockVpnProvisioner {
        async fn provision_node(
            &self,
            node: &Node,
            _user: &User,
            devices: SubscriptionDevices,
        ) -> DomainResult<()> {
            self.calls.lock().unwrap().push((node.name().to_string(), devices));
            if self.fail_for.contains(&node.name().to_string()) {
                return Err(DomainError::SystemFailure("provision failed (test)".into()));
            }
            Ok(())
        }
    }

    struct MockVpnConfigGenerator {
        output: String,
        calls: Arc<Mutex<Vec<(Vec<String>, Uuid)>>>,
    }

    impl VpnConfigGenerator for MockVpnConfigGenerator {
        fn generate(&self, nodes: &[&Node], user_uuid: Uuid) -> DomainResult<String> {
            let names: Vec<String> = nodes.iter().map(|n| n.name().to_string()).collect();
            self.calls.lock().unwrap().push((names, user_uuid));
            Ok(self.output.clone())
        }
    }

    // ==========================================
    // Хелперы
    // ==========================================

    fn make_user(id: i64) -> User {
        let mut user = User::new(
            TelegramId::new(1000 + id),
            Uuid::new_v4(),
            Some(format!("user{id}")),
            format!("User {id}"),
            Money::new(15000).unwrap(),
            ReferralCode::new(format!("REF{id}")),
            SubscriptionToken::new(format!("TOK{id}")),
        );
        user.assign_id(UserId::new(id));
        user
    }

    fn make_active_subscription(user_id: i64, devices: i32) -> Subscription {
        let now = Utc::now();
        Subscription::new(
            UserId::new(user_id),
            SubscriptionPlan::Month1,
            now,
            now + Days::new(30),
            SubscriptionStatus::Active,
            SubscriptionDevices::new(devices).unwrap(),
        )
    }

    fn make_node(id: i64, name: &str) -> Node {
        Node::restore_from_db(
            NodeId::new(id),
            name.to_string(),
            NodeIpAddress::from_str("203.0.113.10").unwrap(),
            true,
            Utc::now(),
        )
    }

    #[allow(clippy::type_complexity)]
    fn setup(
        active_subscription: Option<Subscription>,
        nodes: Vec<Node>,
        fail_provision_for: Vec<String>,
        fail_upsert_for: Vec<NodeId>,
        config_output: &str,
    ) -> (
        IssueVpnConfigCommand,
        Arc<Mutex<Vec<VpnConnection>>>,
        Arc<Mutex<Vec<(String, SubscriptionDevices)>>>,
        Arc<Mutex<Vec<(Vec<String>, Uuid)>>>,
    ) {
        let sub_repo = Arc::new(MockSubscriptionRepository {
            active: active_subscription,
        });
        let node_repo = Arc::new(MockNodeRepository { nodes });
        let upserted = Arc::new(Mutex::new(Vec::new()));
        let vpn_connection_repo = Arc::new(MockVpnConnectionRepository {
            fail_for: fail_upsert_for,
            upserted: upserted.clone(),
        });
        let provision_calls = Arc::new(Mutex::new(Vec::new()));
        let vpn_provisioner = Arc::new(MockVpnProvisioner {
            fail_for: fail_provision_for,
            calls: provision_calls.clone(),
        });
        let generator_calls = Arc::new(Mutex::new(Vec::new()));
        let config_generator = Arc::new(MockVpnConfigGenerator {
            output: config_output.to_string(),
            calls: generator_calls.clone(),
        });
        let cmd = IssueVpnConfigCommand::new(
            sub_repo,
            node_repo,
            vpn_connection_repo,
            vpn_provisioner,
            config_generator,
        );
        (cmd, upserted, provision_calls, generator_calls)
    }

    // ==========================================
    // Тесты
    // ==========================================

    #[tokio::test]
    async fn test_user_without_id_returns_entity_not_saved() {
        let user = User::new(
            TelegramId::new(1),
            Uuid::new_v4(),
            Some("nobody".to_string()),
            "Nobody".to_string(),
            Money::new(15000).unwrap(),
            ReferralCode::new("REF1".to_string()),
            SubscriptionToken::new("TOK1".to_string()),
        ); // assign_id() не вызывался -> id() == None
        let (cmd, upserted, provision_calls, generator_calls) =
            setup(None, vec![], vec![], vec![], "");

        let result = cmd.execute(&user).await;

        assert!(matches!(
            result,
            Err(AppError::Domain(DomainError::User(EntityNotSaved)))
        ));
        assert!(provision_calls.lock().unwrap().is_empty());
        assert!(upserted.lock().unwrap().is_empty());
        assert!(generator_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_no_active_subscription_returns_error() {
        let user = make_user(1);
        let (cmd, upserted, provision_calls, generator_calls) =
            setup(None, vec![make_node(1, "de-1")], vec![], vec![], "");

        let result = cmd.execute(&user).await;

        assert!(matches!(
            result,
            Err(AppError::Domain(DomainError::Subscription(
                NoActiveSubscription
            )))
        ));
        assert!(
            provision_calls.lock().unwrap().is_empty(),
            "без подписки провижининг не должен запускаться вообще"
        );
        assert!(upserted.lock().unwrap().is_empty());
        assert!(generator_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_no_active_nodes_returns_internal_error() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 3);
        let (cmd, upserted, provision_calls, generator_calls) =
            setup(Some(sub), vec![], vec![], vec![], "");

        let result = cmd.execute(&user).await;

        assert!(
            matches!(result, Err(AppError::Internal(_))),
            "если активных нод нет вообще - это внутренняя проблема сервиса, а не юзера"
        );
        assert!(provision_calls.lock().unwrap().is_empty());
        assert!(upserted.lock().unwrap().is_empty());
        assert!(generator_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_happy_path_provisions_all_nodes_and_generates_config() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 5);
        let node1 = make_node(10, "nl-1");
        let node2 = make_node(20, "de-1");
        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![node1, node2],
            vec![],
            vec![],
            "proxies: [...]",
        );

        let result = cmd.execute(&user).await.unwrap();

        assert_eq!(result, "proxies: [...]");

        let calls = provision_calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            2,
            "провижининг должен вызываться для каждой активной ноды"
        );
        let expected_devices = SubscriptionDevices::new(5).unwrap();
        assert!(calls.contains(&("nl-1".to_string(), expected_devices)));
        assert!(calls.contains(&("de-1".to_string(), expected_devices)));
        drop(calls);

        let saved = upserted.lock().unwrap();
        assert_eq!(
            saved.len(),
            2,
            "оба успешных подключения должны попасть в БД"
        );
        assert!(
            saved.iter().all(VpnConnection::is_synced),
            "подключение обязано быть помечено синхронизированным ДО записи в БД"
        );
        drop(saved);

        let gen_calls = generator_calls.lock().unwrap();
        assert_eq!(
            gen_calls.len(),
            1,
            "генератор конфига должен вызываться ровно один раз"
        );
        assert_eq!(gen_calls[0].0, vec!["nl-1".to_string(), "de-1".to_string()]);
        assert_eq!(gen_calls[0].1, user.uuid());
    }

    #[tokio::test]
    async fn test_provisioning_failure_on_one_node_does_not_break_others() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 2);
        let node_ok = make_node(1, "nl-1");
        let node_bad = make_node(2, "ru-1");
        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![node_bad, node_ok],
            vec!["ru-1".to_string()], // 3x-ui недоступна на этой ноде
            vec![],
            "yaml-with-one-node",
        );

        let result = cmd.execute(&user).await.unwrap();

        assert_eq!(result, "yaml-with-one-node");
        assert_eq!(
            provision_calls.lock().unwrap().len(),
            2,
            "попытка провижининга должна быть для ОБЕИХ нод, несмотря на падение одной"
        );
        let saved = upserted.lock().unwrap();
        assert_eq!(saved.len(), 1, "в БД должна попасть только успешная нода");
        assert_eq!(saved[0].node_id(), NodeId::new(1));
        drop(saved);
        assert_eq!(
            generator_calls.lock().unwrap()[0].0,
            vec!["nl-1".to_string()],
            "в конфиг должна попасть только живая нода"
        );
    }

    #[tokio::test]
    async fn test_all_nodes_fail_provisioning_returns_error_without_calling_generator() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 2);
        let node1 = make_node(1, "nl-1");
        let node2 = make_node(2, "ru-1");
        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![node1, node2],
            vec!["nl-1".to_string(), "ru-1".to_string()],
            vec![],
            "unused",
        );

        let result = cmd.execute(&user).await;

        assert!(matches!(result, Err(AppError::Internal(_))));
        assert_eq!(
            provision_calls.lock().unwrap().len(),
            2,
            "попытки были для обеих нод"
        );
        assert!(upserted.lock().unwrap().is_empty());
        assert!(
            generator_calls.lock().unwrap().is_empty(),
            "если провижининг не удался нигде - генерировать конфиг нечего"
        );
    }

    #[tokio::test]
    async fn test_db_upsert_failure_excludes_node_from_config_but_keeps_vpn_access() {
        // Фиксируем осознанное текущее поведение: если апсерт VpnConnection в БД
        // падает ПОСЛЕ того как provision_node уже реально включил доступ на ноде -
        // нода тихо не попадает ни в БД, ни в сгенерированный конфиг. Дыры в
        // безопасности тут нет (VpnAccessRevoker опрашивает панели напрямую по tg_id,
        // а не смотрит в vpn_connections), но юзер в моменте не получит один из
        // своих серверов, пока БД не поправится на следующий клик "Мой VPN".
        let user = make_user(1);
        let sub = make_active_subscription(1, 2);
        let node_ok = make_node(1, "nl-1");
        let node_db_broken = make_node(2, "ru-1");
        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![node_ok, node_db_broken],
            vec![],               // на уровне 3x-ui обе ноды отработали успешно
            vec![NodeId::new(2)], // а запись в БД для второй ноды упала
            "yaml-with-one-node",
        );

        let result = cmd.execute(&user).await.unwrap();

        assert_eq!(result, "yaml-with-one-node");
        assert_eq!(
            provision_calls.lock().unwrap().len(),
            2,
            "3x-ui провижинил обе ноды"
        );
        let saved = upserted.lock().unwrap();
        assert_eq!(
            saved.len(),
            1,
            "в БД осела только одна из двух реально выданных нод"
        );
        drop(saved);
        assert_eq!(
            generator_calls.lock().unwrap()[0].0,
            vec!["nl-1".to_string()],
            "юзер получит конфиг только с одной нодой, хотя доступ выдан на обе"
        );
    }

    #[tokio::test]
    async fn test_db_upsert_failure_on_all_nodes_returns_error() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 2);
        let node1 = make_node(1, "nl-1");
        let node2 = make_node(2, "ru-1");
        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![node1, node2],
            vec![],
            vec![NodeId::new(1), NodeId::new(2)],
            "unused",
        );

        let result = cmd.execute(&user).await;

        assert!(matches!(result, Err(AppError::Internal(_))));
        assert_eq!(provision_calls.lock().unwrap().len(), 2);
        assert!(upserted.lock().unwrap().is_empty());
        assert!(generator_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_node_without_id_is_skipped_and_command_succeeds() {
        let user = make_user(1);
        let sub = make_active_subscription(1, 2);
        let good_node = make_node(1, "nl-1");
        let broken_node = Node::new(
            "no-id-node".to_string(),
            NodeIpAddress::from_str("203.0.113.20").unwrap(),
        );

        let (cmd, upserted, provision_calls, generator_calls) = setup(
            Some(sub),
            vec![good_node, broken_node],
            vec![],
            vec![],
            "mocked_config_string",
        );

        let result = cmd.execute(&user).await;

        assert!(
            result.is_ok(),
            "Команда должна завершиться успешно, пропустив сломанную ноду"
        );
        assert_eq!(
            provision_calls.lock().unwrap().len(),
            2,
            "Провижининг должен выполниться для обеих нод"
        );
        assert_eq!(
            upserted.lock().unwrap().len(),
            1,
            "Стейт в БД должен сохраниться только для good_node"
        );

        let gen_calls = generator_calls.lock().unwrap();

        assert_eq!(gen_calls.len(), 1, "Генератор должен быть вызван один раз");

        let (names, _) = &gen_calls[0];

        assert_eq!(
            names.len(),
            1,
            "Генератору должен быть передан список из одной здоровой ноды"
        );
        assert_eq!(names[0], "nl-1");
    }
}
