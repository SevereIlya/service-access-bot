use crate::adapters::telegram::{BotState, Command, TelegramResult, message_error, views};
use crate::application::{AppliedReferral, Registration};
use crate::domain::user::{ReferralCode, User};
use teloxide::prelude::*;
use teloxide::types::ParseMode::Html;
use tracing::{error, info};

pub async fn handle_commands(
    bot: Bot,
    msg: Message,
    state: BotState,
    user_opt: Option<User>,
    cmd: Command,
) -> TelegramResult<()> {
    let chat_id = msg.chat.id;
    let telegram_id = chat_id.0;

    match cmd {
        Command::Start(payload) => {
            let username: Option<String> = msg.chat.username().map(ToString::to_string);
            let full_name: String = [msg.chat.first_name(), msg.chat.last_name()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");

            match state.usecases.register_user.execute(telegram_id, username, full_name).await {
                Ok(registration) => {
                    let applied =
                        try_apply_referral(&state, &registration, telegram_id, &payload).await;
                    let user = registration.into_user();

                    let menu_state = state.usecases.get_menu_state.execute(&user).await?;
                    let view = views::build_start_message(&state.ui_text, menu_state.can_trial);
                    bot.send_message(chat_id, view.text)
                        .parse_mode(Html)
                        .reply_markup(view.keyboard)
                        .await?;

                    if let Some(applied) = applied {
                        let view = views::build_referral_welcome_message(
                            &state.ui_text,
                            applied.welcome_bonus,
                        );
                        bot.send_message(chat_id, view.text)
                            .parse_mode(Html)
                            .reply_markup(view.keyboard)
                            .await?;
                    }
                }
                Err(e) => {
                    error!(error = ?e, "Ошибка при обработке /start");
                    let text = message_error(&state.ui_text, &e);
                    bot.send_message(chat_id, text).parse_mode(Html).await?;
                }
            }
        }
        Command::Admin => {
            if let Some(user) = user_opt {
                if user.is_admin() {
                    // let admin_stats = stats(pool).await.unwrap_or_default();
                    // let view = build_admin_dashboard(&admin_stats);
                    // bot.send_message(chat_id, view.text)
                    //     .reply_markup(view.keyboard)
                    //     .parse_mode(Html)
                    //     .await?;
                } else {
                    bot.send_message(chat_id, "Команда не найдена.").await?;
                }
            } else {
                bot.send_message(chat_id, "❌ Сначала напиши /start").await?;
            }
        }
    }
    Ok(())
}

fn parse_referral_payload(payload: &str) -> Option<ReferralCode> {
    let payload = payload.trim();
    if payload.is_empty() {
        return None;
    }
    match ReferralCode::parse(payload) {
        Ok(code) => Some(code),
        Err(e) => {
            info!(
                error = ?e,
                payload_len = payload.len(),
                "Невалидный payload в /start, реферальный код проигнорирован"
            );
            None
        }
    }
}

async fn try_apply_referral(
    state: &BotState,
    registration: &Registration,
    telegram_id: i64,
    payload: &str,
) -> Option<AppliedReferral> {
    let code = parse_referral_payload(payload)?;
    match state.usecases.apply_referral.execute(registration, &code).await {
        Ok(applied) => applied,
        Err(e) => {
            error!(
                error = ?e,
                telegram_id,
                referral_code = code.inner(),
                "Не удалось применить реферальный код при /start: юзер остался без бонуса"
            );
            None
        }
    }
}
