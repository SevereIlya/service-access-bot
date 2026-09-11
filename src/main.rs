use telegram_bot::adapters::start_scheduler;
use telegram_bot::adapters::telegram::start_bot;
use telegram_bot::adapters::web::start_web_server;
use telegram_bot::bootstrap::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app = AppState::build().await?;

    tokio::spawn(start_scheduler(app.bot_state.usecases.clone()));
    tokio::spawn(start_web_server(app.web_state, app.web_port));

    start_bot(app.bot, app.bot_state).await;

    Ok(())
}
