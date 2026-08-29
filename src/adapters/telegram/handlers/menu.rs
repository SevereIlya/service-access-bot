use crate::adapters::telegram::callbacks::MenuAction;
use crate::adapters::telegram::error::TelegramResult;
use crate::adapters::telegram::views::message_error;
use crate::adapters::telegram::{BotState, views};
use crate::application::error::AppError;
use crate::domain::error::DomainError;
use crate::domain::error::UserError::NotFound;
use teloxide::prelude::*;
use teloxide::types::{InputFile, MaybeInaccessibleMessage, ParseMode::Html};
use tracing::warn;

#[allow(clippy::match_same_arms)] // Временно
pub async fn handle_menu(
    bot: Bot,
    _qry: &CallbackQuery,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    action: MenuAction,
) -> TelegramResult<()> {
    match action {
        MenuAction::StartTrial => handle_start_trial(bot, msg, state).await?,
        MenuAction::Router => handle_my_vpn(bot, msg, state).await?,
        MenuAction::Profile => {}
        MenuAction::Tariffs => {}
        MenuAction::Referral => {}
        MenuAction::Help => {}
        MenuAction::Down => show_main_menu(bot, msg, state, true).await?,
        MenuAction::Main => show_main_menu(bot, msg, state, false).await?,
    }
    Ok(())
}

// ============================================================================================== //

pub async fn handle_start_trial(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let telegram_id = chat_id.0;

    let Some(user) = state.usecases.get_user.execute(telegram_id).await? else {
        // Отсутствие пользователя - это ошибка, но мы ее расцениваем как бизнес-сценарий.
        // Мы хотим отправить юзеру сообщение. Отправив это сообщение, мы успешно обработали
        // ситуацию с точки зрения бота. Поэтому возвращаем Ok(())
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(&state.ui, &AppError::Domain(DomainError::User(NotFound)));
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    match state.usecases.start_trial.execute(user).await {
        Ok(subscription) => {
            let view = views::build_trial_success_view(&state.ui, subscription.expires_at());

            let is_media = msg
                .regular_message()
                .is_some_and(|m| m.photo().is_some() || m.document().is_some());

            if is_media {
                let _ = bot.delete_message(chat_id, message_id).await;
                bot.send_message(chat_id, view.text)
                    .parse_mode(Html)
                    .reply_markup(view.keyboard)
                    .await?;
            } else {
                bot.edit_message_text(chat_id, message_id, view.text)
                    .parse_mode(Html)
                    .reply_markup(view.keyboard)
                    .await?;
            }
        }
        Err(e) => {
            warn!(error = ?e, telegram_id, "Отказ в выдаче триала");
            let text = message_error(&state.ui, &e);
            bot.send_message(chat_id, text).parse_mode(Html).await?;
        }
    }
    Ok(())
}

pub async fn handle_my_vpn(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let telegram_id = chat_id.0;

    let Some(user) = state.usecases.get_user.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(&state.ui, &AppError::Domain(DomainError::User(NotFound)));
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    // Опционально: можно кинуть сообщение "Генерирую конфиг, подождите...",
    // потому что походы по HTTP на ноды могут занять 2-3 секунды.
    // Но пока просто идем в юзкейс.

    match state.usecases.issue_vpn_config.execute(&user).await {
        Ok(yaml_string) => {
            let view = views::build_vpn_issued_view(&state.ui);

            let _ = bot.delete_message(chat_id, message_id).await;

            let file_name = format!("ParalinkVPN_{}.yaml", user.telegram_id().inner());

            bot.send_document(
                chat_id,
                InputFile::memory(yaml_string.into_bytes()).file_name(file_name),
            )
            .caption(view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
        }
        Err(e) => {
            warn!(error = ?e, telegram_id, "Не удалось выдать VPN-конфиг");
            let text = message_error(&state.ui, &e);
            bot.send_message(chat_id, text).parse_mode(Html).await?;
        }
    }

    Ok(())
}

pub async fn show_main_menu(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    drop_down: bool,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let message_id = msg.id();
    let telegram_id = chat_id.0;

    let Some(user) = state.usecases.get_user.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(&state.ui, &AppError::Domain(DomainError::User(NotFound)));
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    let menu_state = state.usecases.get_menu_state.execute(&user).await?;

    let view = if drop_down {
        views::build_refresh_menu_view(&state.ui, menu_state.can_trial)
    } else {
        views::build_main_menu_view(&state.ui, menu_state.can_trial)
    };

    let is_media = msg
        .regular_message()
        .is_some_and(|m| m.photo().is_some() || m.document().is_some());

    if drop_down || is_media {
        let _ = bot.delete_message(chat_id, message_id).await;
        bot.send_message(chat_id, view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
    } else {
        bot.edit_message_text(chat_id, message_id, view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
    }
    Ok(())
}
