use crate::adapters::web::{WebState, handle_platega_webhook, handle_subscription_profile};
use axum::Router;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tracing::info;

pub async fn start_web_server(state: WebState, port: u16) -> anyhow::Result<()> {
    let router = Router::new()
        .route("/sub/{token}", get(handle_subscription_profile))
        .route("/webhook/platega", post(handle_platega_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!(%addr, "Веб-адаптер axum поднят");

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
