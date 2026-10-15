use crate::adapters::telegram::{UiText, format_rub};
use crate::domain::{
    DomainError, DomainResult,
    billing::Money,
    device::{DeviceName, DeviceSetupCode},
    notification::Notifier,
    user::User,
};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use std::sync::Arc;
use teloxide::utils::html::escape;
use teloxide::{
    ApiError, RequestError,
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode::Html},
};
use tracing::info;

/// Отправитель Telegram-сообщений пользователям.
///
/// Отправляет HTML-сообщения и обрабатывает ошибки Telegram API.
pub struct TelegramNotifier {
    bot: Bot,
    ui: Arc<UiText>,
}

impl TelegramNotifier {
    #[must_use]
    pub const fn new(bot: Bot, ui: Arc<UiText>) -> Self {
        Self { bot, ui }
    }

    /// Отправляет пользователю HTML-сообщение.
    ///
    /// Если пользователь заблокировал бота или его аккаунт деактивирован,
    /// ошибка Telegram API считается ожидаемым результатом:
    /// метод записывает лог и возвращает `Ok(())`.
    async fn send_html(
        &self,
        user: &User,
        text: String,
        keyboard: Option<InlineKeyboardMarkup>,
    ) -> DomainResult<()> {
        let chat_id = ChatId(user.telegram_id().inner());
        let mut request = self.bot.send_message(chat_id, text).parse_mode(Html);
        if let Some(keyboard) = keyboard {
            request = request.reply_markup(keyboard);
        }

        match request.await {
            Ok(_) => Ok(()),
            Err(RequestError::Api(ApiError::BotBlocked)) => {
                info!(telegram_id = %user.telegram_id(), "Юзер заблокировал бота.");
                Ok(())
            }
            Err(RequestError::Api(ApiError::UserDeactivated)) => {
                info!(telegram_id = %user.telegram_id(), "Аккаунт юзера деактивирован.");
                Ok(())
            }
            Err(e) => Err(DomainError::SystemFailure(e.to_string())),
        }
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
        let date_str = expires_at.with_timezone(&msk_offset).format("%d.%m.%Y %H:%M").to_string();
        let text = self.ui.message.msg_subscription_expiring.replace("{date}", &date_str);

        self.send_html(user, text, None).await
    }

    async fn notify_subscription_expired(&self, user: &User) -> DomainResult<()> {
        let text = self.ui.message.msg_subscription_expired.clone();

        self.send_html(user, text, None).await
    }

    async fn notify_balance_topped_up(
        &self,
        user: &User,
        amount: Money,
        new_balance: Money,
    ) -> DomainResult<()> {
        let text = self
            .ui
            .message
            .msg_balance_topped_up
            .replace("{amount}", &format_rub(amount))
            .replace("{balance}", &format_rub(new_balance));
        let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
            self.ui.button.btn_menu_tariffs.clone(),
            "menu:tariffs",
        )]]);

        self.send_html(user, text, Some(keyboard)).await
    }

    async fn notify_new_device_detected(
        &self,
        user: &User,
        device_name: &DeviceName,
    ) -> DomainResult<()> {
        let text = self
            .ui
            .message
            .msg_new_device_detected
            .replace("{device}", &escape(device_name.inner()));

        self.send_html(user, text, None).await
    }

    async fn notify_device_setup_code(
        &self,
        user: &User,
        device_name: &DeviceName,
        code: &DeviceSetupCode,
    ) -> DomainResult<()> {
        // Порядок важен
        let text = self
            .ui
            .message
            .msg_device_setup_code
            .replace("{code}", code.inner())
            .replace("{device}", &escape(device_name.inner()));

        self.send_html(user, text, None).await
    }
}
