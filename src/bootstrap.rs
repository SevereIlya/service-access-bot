use crate::adapters::{
    telegram::{BotState, TelegramNotifier, UiText},
    web::WebState,
};
use crate::application::UseCases;
use crate::domain::{
    billing::{DynPaymentGateway, DynTransactionRepository, Money},
    notification::DynNotifier,
    subscription::DynSubscriptionRepository,
    uow::DynUnitOfWork,
    user::DynUserRepository,
    vpn::{
        DynNodeRepository, DynVpnAccessRevoker, DynVpnConfigGenerator, DynVpnConnectionRepository,
        DynVpnProvisioner,
    },
};
use crate::infrastructure::{
    AppConfig, ClashConfigGenerator, PlategaGateway, SqlxNodeRepository,
    SqlxSubscriptionRepository, SqlxTransactionRepository, SqlxUnitOfWork, SqlxUserRepository,
    SqlxVpnConnectionRepository, XuiAccessRevoker, XuiProvisioner,
};
use std::sync::Arc;
use teloxide::prelude::*;
use tracing::{debug, info};

pub struct AppState {
    pub bot: Bot,
    pub bot_state: BotState,
    pub web_state: WebState,
    pub web_port: u16,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let config = Arc::new(AppConfig::load()?);
        let ui_text = Arc::new(UiText::load("locales/ru.toml")?);
        info!("Конфигурация загружена");

        let pool =
            crate::infrastructure::postgres::create_pg_pool(&config.general.database_url).await?;
        info!("Подключение к БД установлено");

        let user_repo: DynUserRepository = Arc::new(SqlxUserRepository::new(pool.clone()));
        let subscription_repo: DynSubscriptionRepository =
            Arc::new(SqlxSubscriptionRepository::new(pool.clone()));
        let node_repo: DynNodeRepository = Arc::new(SqlxNodeRepository::new(pool.clone()));
        let vpn_connection_repo: DynVpnConnectionRepository =
            Arc::new(SqlxVpnConnectionRepository::new(pool.clone()));
        let transaction_repo: DynTransactionRepository =
            Arc::new(SqlxTransactionRepository::new(pool.clone()));
        let uow: DynUnitOfWork = Arc::new(SqlxUnitOfWork::new(pool.clone()));
        debug!("Инициализация репозиториев завершена");

        let bot = Bot::new(&config.general.telegram_token);
        let me = bot.get_me().await.map_err(|e| {
            anyhow::anyhow!("Не удалось получить информацию о боте от Telegram: {e}")
        })?;
        let bot_username = me.username().to_string();
        info!(username = %bot_username, "Авторизация в Telegram успешна");

        let notifier: DynNotifier = Arc::new(TelegramNotifier::new(bot.clone(), ui_text.clone()));
        let vpn_revoker: DynVpnAccessRevoker = Arc::new(XuiAccessRevoker::new(&config)?);
        let vpn_provisioner: DynVpnProvisioner = Arc::new(XuiProvisioner::new(&config)?);
        let config_generator: DynVpnConfigGenerator =
            Arc::new(ClashConfigGenerator::new(config.clone()));
        info!("VPN-адаптеры инициализированы");

        let payment_gateway: DynPaymentGateway = Arc::new(PlategaGateway::new(config.payments.platega.clone())?);
        info!("PlategaGateway инициализирован");

        debug!("Инициализация юзкейсов приложения");
        let usecases = Arc::new(UseCases::new(
            user_repo,
            subscription_repo,
            node_repo,
            vpn_connection_repo,
            transaction_repo,
            uow,
            vpn_revoker,
            vpn_provisioner,
            config_generator,
            payment_gateway,
            notifier,
            config.general.uuid_namespace,
            Money::new(config.payments.base_price)?,
        ));

        let bot_state = BotState {
            usecases: usecases.clone(),
            bot_username,
            ui_text,
            admin_chat_id: config.general.admin_chat_id,
            public_base_url: config.general.public_base_url.clone(),
        };

        let web_state = WebState {
            usecases,
            config: config.clone(),
        };

        Ok(Self {
            bot,
            bot_state,
            web_state,
            web_port: config.general.web_port,
        })
    }
}
