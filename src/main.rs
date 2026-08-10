use telegram_bot::adapters::telegram::router::start_bot;
use telegram_bot::bootstrap::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app = AppState::build().await?;

    start_bot(app.bot, app.bot_state).await;

    Ok(())
}
