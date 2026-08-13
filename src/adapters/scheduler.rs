use crate::application::usecases::UseCases;
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{error, info, instrument};

const TICK_INTERVAL: Duration = Duration::from_hours(1);

#[instrument(skip_all)]
pub async fn start_scheduler(usecases: Arc<UseCases>) {
    let mut timer = interval(TICK_INTERVAL);
    loop {
        timer.tick().await;
        run_tick(&usecases).await;
    }
}

async fn run_tick(usecases: &UseCases) {
    info!("Запуск проверки подписок");

    match usecases.expire_lapsed_subscriptions.execute().await {
        Ok(summary) => info!(
            expired = summary.expired,
            failed = summary.failed,
            "проверка истёкших подписок завершена"
        ),
        Err(e) => error!(error = ?e, "не удалось выполнить проверку истёкших подписок"),
    }

    match usecases.send_expiry_warnings.execute().await {
        Ok(summary) => info!(
            warned = summary.warned,
            failed = summary.failed,
            "рассылка уведомлений завершена"
        ),
        Err(e) => error!(error = ?e, "не удалось разослать предупреждения"),
    }
}
