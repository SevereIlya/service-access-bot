use crate::adapters::telegram::{
    BotState, PurchaseAction, TelegramResult, message_error, render_view, views,
};
use crate::application::AppError;
use crate::domain::{
    BillingError, DomainError,
    UserError::NotFound,
    subscription::{SubscriptionDevices, SubscriptionPlan},
    user::User,
};
use reqwest::Url;
use teloxide::{prelude::*, types::MaybeInaccessibleMessage, types::ParseMode::Html};
use tracing::{info, warn};

async fn require_user(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
) -> TelegramResult<Option<User>> {
    let telegram_id = chat_id.0;
    let Some(user) = state.usecases.get_user_by_telegram_id.execute(telegram_id).await? else {
        warn!(error = ?DomainError::User(NotFound), telegram_id, "Пользователь не найден");
        let text = message_error(
            &state.ui_text,
            &AppError::Domain(DomainError::User(NotFound)),
        );
        bot.send_message(chat_id, text).await?;
        return Ok(None);
    };
    Ok(Some(user))
}

pub async fn handle_tariffs(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    devices: i32,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let Some(user) = require_user(&bot, chat_id, &state).await? else {
        return Ok(());
    };
    let view = views::build_tariffs_view(&state.ui_text, &user, devices)?;
    render_view(&bot, &msg, view).await
}

pub async fn handle_purchase(
    bot: Bot,
    qry: &CallbackQuery,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    action: PurchaseAction,
) -> TelegramResult<()> {
    bot.answer_callback_query(qry.id.clone()).await?;
    match action {
        PurchaseAction::View(plan, devices) => {
            show_purchase_quote(bot, msg, state, plan, devices).await?;
        }
        PurchaseAction::Pay(plan, devices) => {
            handle_pay(bot, msg, state, plan, devices).await?;
        }
        PurchaseAction::TopUp(plan, devices) => {
            handle_top_up(bot, msg, state, plan, devices).await?;
        }
    }
    Ok(())
}

async fn show_purchase_quote(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    plan: SubscriptionPlan,
    devices: i32,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let Some(user) = require_user(&bot, chat_id, &state).await? else {
        return Ok(());
    };
    let quote = state.usecases.get_purchase_quote.execute(&user, plan, devices).await?;
    let view = views::build_purchase_quote_view(&state.ui_text, plan, devices, &quote);
    render_view(&bot, &msg, view).await
}

async fn handle_pay(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    plan: SubscriptionPlan,
    devices: i32,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let Some(user) = require_user(&bot, chat_id, &state).await? else {
        return Ok(());
    };
    let devices_vo = SubscriptionDevices::new(devices).map_err(AppError::from)?;

    match state.usecases.purchase_tariff.execute(&user, plan, devices_vo).await {
        Ok(sub) => {
            let view = views::build_purchase_success_view(
                &state.ui_text,
                sub.plan(),
                sub.devices(),
                sub.expires_at(),
            );
            render_view(&bot, &msg, view).await
        }
        Err(AppError::Domain(DomainError::Billing(BillingError::InsufficientBalance {
            ..
        }))) => {
            info!(
                ?plan,
                devices, "Баланс изменился между показом и оплатой, перерисовываю"
            );
            show_purchase_quote(bot, msg, state, plan, devices).await
        }
        Err(e) => {
            warn!(error = ?e, ?plan, devices, "Не удалось купить тариф");
            let text = message_error(&state.ui_text, &e);
            bot.send_message(chat_id, text).parse_mode(Html).await?;
            Ok(())
        }
    }
}

async fn handle_top_up(
    bot: Bot,
    msg: MaybeInaccessibleMessage,
    state: BotState,
    plan: SubscriptionPlan,
    devices: i32,
) -> TelegramResult<()> {
    let chat_id = msg.chat().id;
    let Some(user) = require_user(&bot, chat_id, &state).await? else {
        return Ok(());
    };
    let quote = state.usecases.get_purchase_quote.execute(&user, plan, devices).await?;

    let Some(shortfall) = quote.shortfall else {
        return show_purchase_quote(bot, msg, state, plan, devices).await;
    };

    let payment_url = state.usecases.request_top_up.execute(&user, shortfall).await?;
    let url = Url::parse(&payment_url)
        .map_err(|e| AppError::Internal(format!("Platega вернула невалидный URL: {e}")))?;

    let view = views::build_top_up_link_view(&state.ui_text, url, shortfall, plan, devices);
    render_view(&bot, &msg, view).await
}
