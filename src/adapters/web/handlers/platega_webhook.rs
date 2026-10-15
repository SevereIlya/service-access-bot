use crate::adapters::web::{WebError, WebResult, WebState};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use std::net::IpAddr;
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct PlategaWebhookPayload {
    pub id: String,
}

pub async fn handle_platega_webhook(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(payload): Json<PlategaWebhookPayload>,
) -> WebResult<StatusCode> {
    let real_ip = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<IpAddr>().ok());

    let allowed_ips = &state.config.payments.platega.webhook_allowed_ip;

    if !real_ip.is_some_and(|ip| allowed_ips.contains(&ip)) {
        error!(?real_ip, expected = ?allowed_ips, "Вебхук Платеги с левого IP");
        return Err(WebError::Forbidden);
    }

    state.usecases.process_payment_webhook.execute(&payload.id).await?;
    Ok(StatusCode::OK)
}
