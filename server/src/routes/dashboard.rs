use axum::extract::State;
use axum::routing::get;
use axum::Json;
use axum::Router;
use serde::Deserialize;

use crate::auth::JwtUser;
use crate::error::ProxyError;
use crate::AppState;

#[derive(Debug, Deserialize)]
struct UsageGraphQuery {
    from: String,
    to: String,
    #[serde(default)]
    granularity: Option<model::usage_log::Granularity>,
    model: Option<String>,
}

async fn usage_graph_handler(
    State(state): State<AppState>,
    user: JwtUser,
    axum::extract::Query(params): axum::extract::Query<UsageGraphQuery>,
) -> Result<Json<Vec<model::usage_log::UsageGraphPoint>>, ProxyError> {
    let from = chrono::DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", params.from))
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| ProxyError::BadRequest(format!("invalid 'from' date: {e}")))?;

    let to = chrono::DateTime::parse_from_rfc3339(&format!("{}T23:59:59Z", params.to))
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| ProxyError::BadRequest(format!("invalid 'to' date: {e}")))?;

    let granularity = params.granularity.unwrap_or_default();

    let points = state
        .repo
        .usage_graph(&user.user_id, from, to, granularity, params.model.as_deref())
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query usage graph: {e}")))?;

    Ok(Json(points))
}

async fn list_models_handler(
    State(state): State<AppState>,
    user: JwtUser,
) -> Result<Json<Vec<String>>, ProxyError> {
    let models = state
        .repo
        .list_models(&user.user_id)
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to list models: {e}")))?;

    Ok(Json(models))
}

pub fn dashboard_router() -> Router<AppState> {
    Router::new()
        .route("/usage/graph", get(usage_graph_handler))
        .route("/usage/models", get(list_models_handler))
}
