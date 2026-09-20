use crate::adapters::telegram::{
    BotState, MenuAction, TelegramResult,
    handlers::{profile::handle_profile, purchase::handle_tariffs, render_view},
    message_error, views,
};
use crate::application::AppError;
use crate::domain::{DomainError, UserError::NotFound};
use image::Luma;
use qrcode::QrCode;
use teloxide::{
    prelude::*,
    types::{InputFile, MaybeInaccessibleMessage, ParseMode::Html},
};
use tracing::warn;

pub async fn handle_menu(
    bot: Bot,
    _qry: &CallbackQuery,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    action: MenuAction,
) -> TelegramResult<()> {
    match action {
        MenuAction::StartTrial => handle_start_trial(bot, msg, state).await?,
        MenuAction::MyVpn => handle_my_vpn(bot, msg, state).await?,
        MenuAction::Profile => handle_profile(bot, msg, state).await?,
        MenuAction::Tariffs(devices) => handle_tariffs(bot, msg, state, devices).await?,
        MenuAction::About => handle_about(bot, msg, state).await?,
        MenuAction::Referral => handle_referral(bot, msg, state).await?,
        MenuAction::Help => handle_help(bot, msg, state).await?,
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
    let telegram_id = chat_id.0;

    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        // Отсутствие пользователя - это ошибка, но мы ее расцениваем как бизнес-сценарий.
        // Мы хотим отправить юзеру сообщение. Отправив это сообщение, мы успешно обработали
        // ситуацию с точки зрения бота. Поэтому возвращаем Ok(())
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(
            &state.ui_text,
            &AppError::Domain(DomainError::User(NotFound)),
        );
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    match state.usecases.start_trial.execute(user).await {
        Ok(subscription) => {
            let view = views::build_trial_success_view(&state.ui_text, subscription.expires_at());
            render_view(&bot, &msg, view).await?;
        }
        Err(e) => {
            warn!(error = ?e, telegram_id, "Отказ в выдаче триала");
            let text = message_error(&state.ui_text, &e);
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

    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(
            &state.ui_text,
            &AppError::Domain(DomainError::User(NotFound)),
        );
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    if let Err(e) = state.usecases.issue_vpn_config.execute(&user).await {
        warn!(error = ?e, telegram_id, "Не удалось выдать VPN-конфиг");
        let text = message_error(&state.ui_text, &e);
        bot.send_message(chat_id, text).parse_mode(Html).await?;
        return Ok(());
    }

    let link = format!(
        "{}/{}",
        state.public_base_url,
        user.subscription_token().inner()
    );
    let view = views::build_vpn_issued_view(&state.ui_text, &link);
    let _ = bot.delete_message(chat_id, message_id).await;

    match build_qr_png(&link) {
        Ok(png) => {
            bot.send_photo(chat_id, InputFile::memory(png).file_name("qr.png"))
                .caption(view.text)
                .parse_mode(Html)
                .reply_markup(view.keyboard)
                .await?;
        }
        Err(e) => {
            warn!(error = %e, "QR не сгенерировался, отправляю только ссылку");
            bot.send_message(chat_id, view.text)
                .parse_mode(Html)
                .reply_markup(view.keyboard)
                .await?;
        }
    }
    Ok(())
}

fn build_qr_png(data: &str) -> anyhow::Result<Vec<u8>> {
    let code = QrCode::new(data.as_bytes())?;
    let img = code.render::<Luma<u8>>().build();
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )?;
    Ok(bytes)
}

pub async fn show_main_menu(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    drop_down: bool,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let telegram_id = chat_id.0;

    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(
            &state.ui_text,
            &AppError::Domain(DomainError::User(NotFound)),
        );
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };

    let menu_state = state.usecases.get_menu_state.execute(&user).await?;

    let view = if drop_down {
        views::build_refresh_menu_view(&state.ui_text, menu_state.can_trial)
    } else {
        views::build_main_menu_view(&state.ui_text, menu_state.can_trial)
    };

    if drop_down {
        let message_id = msg.id();
        let _ = bot.delete_message(chat_id, message_id).await;
        bot.send_message(chat_id, view.text)
            .parse_mode(Html)
            .reply_markup(view.keyboard)
            .await?;
        return Ok(());
    }

    render_view(&bot, &msg, view).await
}

pub async fn handle_about(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let view = views::build_about_view(&state.ui_text);
    render_view(&bot, &msg, view).await
}

pub async fn handle_referral(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let telegram_id = chat_id.0;
    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(&state.ui_text, &AppError::Domain(DomainError::User(NotFound)));
        bot.send_message(chat_id, text).await?;
        return Ok(());
    };
    let view = views::build_referral_view(&state.ui_text, &state.bot_username, user.referral_code().inner());
    render_view(&bot, &msg, view).await
}

pub async fn handle_help(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
) -> TelegramResult<()> {
    let view = views::build_help_view(&state.ui_text);
    render_view(&bot, &msg, view).await
}
