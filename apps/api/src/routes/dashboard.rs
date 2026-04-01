use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde::Deserialize;

use crate::AppState;
use crate::auth::JwtUser;
use crate::response::ApiResponse;
use error::ProxyError;

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
    let seven_days_ago = today - chrono::Duration::days(7);

    let from = match params.from.as_deref() {
        Some(v) => parse_iso_date(v)?,
        None => seven_days_ago.and_hms_opt(0, 0, 0).unwrap().and_utc(),
    };

    let to = match params.to.as_deref() {
        Some(v) => parse_iso_date(v)?,
        None => today.and_hms_opt(23, 59, 59).unwrap().and_utc(),
    };

    let granularity = params.granularity.unwrap_or_default();

    let points = state
        .repo
        .usage_graph(
            &user.user_id,
            from,
            to,
            granularity.clone(),
            params.model.as_deref(),
        )
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query usage graph: {e}")))?;

    let total_points = state
        .repo
        .usage_graph_total(from, to, granularity, params.model.as_deref())
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query total usage: {e}")))?;

    let user_inputs: i64 = points.iter().map(|p| p.inputs).sum();
    let user_outputs: i64 = points.iter().map(|p| p.outputs).sum();
    let user_cache: i64 = points.iter().map(|p| p.cache).sum();

    let all_inputs: i64 = total_points.iter().map(|p| p.inputs).sum();
    let all_outputs: i64 = total_points.iter().map(|p| p.outputs).sum();
    let all_cache: i64 = total_points.iter().map(|p| p.cache).sum();

    let shared = model::usage_log::UsageShared {
        inputs: if all_inputs > 0 {
            (user_inputs as f64 / all_inputs as f64) * 100.0
        } else {
            0.0
        },
        outputs: if all_outputs > 0 {
            (user_outputs as f64 / all_outputs as f64) * 100.0
        } else {
            0.0
        },
        cache: if all_cache > 0 {
            (user_cache as f64 / all_cache as f64) * 100.0
        } else {
            0.0
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_date_rfc3339() {
        let result = parse_iso_date("2026-04-02T10:30:00Z").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-04-02T10:30:00+00:00");
    }

    #[test]
    fn parse_iso_date_date_only() {
        let result = parse_iso_date("2026-04-02").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-04-02T00:00:00+00:00");
    }

    #[test]
    fn parse_iso_date_invalid() {
        let result = parse_iso_date("not-a-date");
        assert!(result.is_err());
    }

    #[test]
    fn parse_iso_date_empty() {
        let result = parse_iso_date("");
        assert!(result.is_err());
    }
}
