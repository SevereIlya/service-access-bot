use telegram_bot::adapters::start_scheduler;
use telegram_bot::adapters::telegram::{init_tracing_with_alerts, start_admin_alerter, start_bot};
use telegram_bot::adapters::web::start_web_server;
use telegram_bot::bootstrap::AppState;

#[rustfmt::skip]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let alert_rx = init_tracing_with_alerts();

    let app = AppState::build().await?;

    tokio::spawn(start_admin_alerter(app.bot.clone(), app.bot_state.clone(), alert_rx));
    tokio::spawn(start_scheduler(app.bot_state.usecases.clone()));
    tokio::spawn(start_web_server(app.web_state, app.web_port));

    start_bot(app.bot, app.bot_state).await;

    Ok(())
}
