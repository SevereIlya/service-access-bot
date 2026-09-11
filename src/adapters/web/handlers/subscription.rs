use crate::adapters::web::{WebError, WebResult, WebState};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use tracing::instrument;

#[instrument(skip(state))]
pub async fn handle_subscription_profile(
    State(state): State<WebState>,
    Path(token): Path<String>,
) -> WebResult<Response> {
    let user = state
        .usecases
        .get_user_by_subscription_token
        .execute(&token)
        .await?
        .ok_or(WebError::NotFound)?;

    let yaml = state.usecases.issue_vpn_config.execute(&user).await?;

    let filename = format!("ParalinkVPN-{}.yaml", user.telegram_id().inner());

    #[rustfmt::skip]
    let headers = [
        (header::CONTENT_TYPE, "text/yaml; charset=utf-8".to_string()),
        (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate".to_string()),
        (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        (header::HeaderName::from_static("profile-update-interval"), "6".to_string()),
    ];

    Ok((headers, yaml).into_response())
}
