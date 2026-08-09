use crate::adapters::telegram::commands::Command;
use crate::adapters::telegram::error::TelegramResult;
use crate::adapters::telegram::views::message_error;
use crate::adapters::telegram::{BotState, views};
use crate::domain::user::User;
use teloxide::prelude::*;
use teloxide::types::ParseMode::Html;
use tracing::error;

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
        Command::Start(_payload) => {
            let username: Option<String> = msg.chat.username().map(ToString::to_string);
            let full_name: String = [msg.chat.first_name(), msg.chat.last_name()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");

            match state.register_user_cmd.execute(telegram_id, username, full_name).await
            {
                Ok(user) => {
                    // TODO: Тут будет проверка рефералки try_process_referral

                    let menu_state = state.get_menu_state_query.execute(&user).await?;
                    let view =
                        views::build_start_message(&state.ui, menu_state.can_trial);
                    bot.send_message(chat_id, view.text)
                        .parse_mode(Html)
                        .reply_markup(view.keyboard)
                        .await?;
                }
                Err(e) => {
                    error!(error = ?e, "Ошибка при обработке /start");
                    let text = message_error(&state.ui, &e);
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
