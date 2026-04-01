use axum::extract::State;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use crate::auth::JwtUser;
use crate::error::ProxyError;
use crate::response::ApiResponse;
use crate::AppState;

#[derive(Debug, Deserialize)]
struct UsageGraphQuery {
    from: Option<String>,
    to: Option<String>,
    #[serde(default)]
    granularity: Option<model::usage_log::Granularity>,
    model: Option<String>,
}

fn parse_iso_date(input: &str) -> Result<chrono::DateTime<chrono::Utc>, ProxyError> {
    // Try full ISO 8601 / RFC 3339 first (e.g. "2026-04-01T00:00:00Z")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }
    // Fall back to date-only (e.g. "2026-04-01")
    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }
    Err(ProxyError::BadRequest(format!("invalid date: {input}")))
}

async fn usage_graph_handler(
    State(state): State<AppState>,
    user: JwtUser,
    axum::extract::Query(params): axum::extract::Query<UsageGraphQuery>,
) -> Result<ApiResponse<model::usage_log::UsageGraphResponse>, ProxyError> {
    let today = chrono::Utc::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);

    let from = match params.from.as_deref() {
        Some(v) => parse_iso_date(v)?,
        None => yesterday.and_hms_opt(0, 0, 0).unwrap().and_utc(),
    };

    let to = match params.to.as_deref() {
        Some(v) => parse_iso_date(v)?,
        None => today.and_hms_opt(23, 59, 59).unwrap().and_utc(),
    };

    let granularity = params.granularity.unwrap_or_default();

    let points = state
        .repo
        .usage_graph(&user.user_id, from, to, granularity, params.model.as_deref())
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query usage graph: {e}")))?;

    let total_inputs: i64 = points.iter().map(|p| p.inputs).sum();
    let total_outputs: i64 = points.iter().map(|p| p.outputs).sum();
    let total_cache: i64 = points.iter().map(|p| p.cache).sum();
    let total = total_inputs + total_outputs + total_cache;

    let shared = if total > 0 {
        model::usage_log::UsageShared {
            inputs: (total_inputs as f64 / total as f64) * 100.0,
            outputs: (total_outputs as f64 / total as f64) * 100.0,
            cache: (total_cache as f64 / total as f64) * 100.0,
        }
    } else {
        model::usage_log::UsageShared {
            inputs: 0.0,
            outputs: 0.0,
            cache: 0.0,
        }
    };

    Ok(ApiResponse::ok(model::usage_log::UsageGraphResponse {
        points,
        shared,
    }))
}

async fn list_models_handler(
    State(state): State<AppState>,
    user: JwtUser,
) -> Result<ApiResponse<Vec<String>>, ProxyError> {
    let models = state
        .repo
        .list_models(&user.user_id)
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to list models: {e}")))?;

    Ok(ApiResponse::ok(models))
}

pub fn dashboard_router() -> Router<AppState> {
    Router::new()
        .route("/usage/graph", get(usage_graph_handler))
        .route("/usage/models", get(list_models_handler))
}
