use crate::adapters::telegram::ui::UiText;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::notification::Notifier;
use crate::domain::user::User;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ParseMode::Html;
use teloxide::{ApiError, RequestError};
use tracing::info;

pub struct TelegramNotifier {
    bot: Bot,
    ui: Arc<UiText>,
}

impl TelegramNotifier {
    #[must_use]
    pub const fn new(bot: Bot, ui: Arc<UiText>) -> Self {
        Self { bot, ui }
    }
}

#[async_trait]
impl Notifier for TelegramNotifier {
    async fn notify_subscription_expiring(
        &self,
        user: &User,
        expires_at: DateTime<Utc>,
    ) -> DomainResult<()> {
        let msk_offset = FixedOffset::east_opt(3 * 3600)
            .expect("Hardcoded offset of +3 hours is mathematically always valid");
        let date_str =
            expires_at.with_timezone(&msk_offset).format("%d.%m.%Y %H:%M").to_string();
        let text = self.ui.message.msg_subscription_expiring.replace("{date}", &date_str);
        let chat_id = ChatId(user.telegram_id().inner());

        match self.bot.send_message(chat_id, text).parse_mode(Html).await {
            Ok(_) => Ok(()),
            Err(RequestError::Api(err)) => {
                if let ApiError::BotBlocked | ApiError::UserDeactivated = err {
                    info!(
                        telegram_id = %user.telegram_id(),
                        "Юзер заблокировал бота."
                    );
                    return Ok(());
                }
                Err(DomainError::SystemFailure(err.to_string()))
            }
            Err(e) => Err(DomainError::SystemFailure(e.to_string())),
        }
    }

    async fn notify_subscription_expired(&self, user: &User) -> DomainResult<()> {
        let chat_id = ChatId(user.telegram_id().inner());
        self.bot
            .send_message(chat_id, self.ui.message.msg_subscription_expired.clone())
            .parse_mode(Html)
            .await
            .map_err(|e| DomainError::SystemFailure(e.to_string()))?;
        Ok(())
    }
}
