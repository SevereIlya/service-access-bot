use crate::adapters::telegram::BotState;
use crate::adapters::telegram::ui::UiText;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::database::create_pg_pool;
use crate::infrastructure::setup::repo::Repositories;
use crate::infrastructure::setup::usecases::UseCases;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use teloxide::prelude::*;
use tracing::info;

mod repo;
mod usecases;

pub struct AppState {
    pub bot: Bot,
    pub bot_state: BotState,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let config = AppConfig::load()?;
        let ui_text = UiText::load("locales/ru.toml")?;
        info!("Конфигурация загружена");

        let pool = create_pg_pool(&config.general.database_url).await?;
        info!("Подключение к БД установлено");

        let repos = Repositories::new(pool);
        let use_cases = UseCases::new(&repos, &config);

        let bot = Bot::new(&config.general.telegram_token);
        let me = bot.get_me().await.map_err(|e| {
            anyhow::anyhow!("Не удалось получить информацию о боте от Telegram: {e}")
        })?;
        let bot_username = me.username().to_string();
        info!(username = %bot_username, "Авторизация в Telegram успешна");

        let bot_state = BotState {
            register_user_cmd: use_cases.register_user,
            start_trial_cmd: use_cases.start_trial,
            get_user_query: use_cases.get_user,
            get_menu_state_query: use_cases.get_menu_state,
            bot_username,
            ui: Arc::new(ui_text),
            user_states: Arc::new(Mutex::new(HashMap::new())),
            broadcasting_admins: Arc::new(Mutex::new(HashSet::new())),
            admin_chat_id: config.general.admin_chat_id,
        };

        Ok(Self { bot, bot_state })
    }
}
