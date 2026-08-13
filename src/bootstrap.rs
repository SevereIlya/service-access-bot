use crate::adapters::telegram::BotState;
use crate::adapters::telegram::notifier::TelegramNotifier;
use crate::adapters::telegram::ui::UiText;
use crate::application::usecases::UseCases;
use crate::domain::notification::DynNotifier;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::uow::DynUnitOfWork;
use crate::domain::user::{DynUserRepository, Money};
use crate::domain::vpn::DynVpnAccessRevoker;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::database::{
    SqlxSubscriptionRepository, SqlxUnitOfWork, SqlxUserRepository, create_pg_pool,
};
use crate::infrastructure::vpn::NoopVpnAccessRevoker;
use std::sync::Arc;
use teloxide::prelude::*;
use tracing::{debug, info};

pub struct AppState {
    pub bot: Bot,
    pub bot_state: BotState,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let config = AppConfig::load()?;
        let ui_text = Arc::new(UiText::load("locales/ru.toml")?);
        info!("Конфигурация загружена");

        let pool = create_pg_pool(&config.general.database_url).await?;
        info!("Подключение к БД установлено");

        let user_repo: DynUserRepository =
            Arc::new(SqlxUserRepository::new(pool.clone()));
        let subscription_repo: DynSubscriptionRepository =
            Arc::new(SqlxSubscriptionRepository::new(pool.clone()));
        let uow: DynUnitOfWork = Arc::new(SqlxUnitOfWork::new(pool.clone()));
        debug!("Инициализация репозиториев завершена");

        let bot = Bot::new(&config.general.telegram_token);
        let me = bot.get_me().await.map_err(|e| {
            anyhow::anyhow!("Не удалось получить информацию о боте от Telegram: {e}")
        })?;
        let bot_username = me.username().to_string();
        info!(username = %bot_username, "Авторизация в Telegram успешна");

        let notifier: DynNotifier =
            Arc::new(TelegramNotifier::new(bot.clone(), ui_text.clone()));
        let vpn_revoker: DynVpnAccessRevoker = Arc::new(NoopVpnAccessRevoker);

        debug!("Инициализация юзкейсов приложения");
        let usecases = Arc::new(UseCases::new(
            user_repo,
            subscription_repo,
            uow,
            vpn_revoker,
            notifier,
            config.general.uuid_namespace,
            Money::new(config.payments.base_price)?,
        ));

        let bot_state = BotState {
            usecases,
            bot_username,
            ui: ui_text,
            user_states: Arc::default(),
            broadcasting_admins: Arc::default(),
            admin_chat_id: config.general.admin_chat_id,
        };

        Ok(Self { bot, bot_state })
    }
}
