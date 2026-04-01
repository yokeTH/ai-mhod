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

#[inline]
fn pct(user: i64, all: i64) -> f64 {
    if all > 0 {
        (user as f64 / all as f64) * 100.0
    } else {
        0.0
    }
}

fn parse_iso_date(input: &str) -> Result<chrono::DateTime<chrono::Utc>, ProxyError> {
    // Try full ISO 8601 / RFC 3339 first (e.g. "2026-04-01T00:00:00Z")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }
    // Fall back to date-only (e.g. "2026-04-01")
    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date
            .and_hms_opt(0, 0, 0)
            .expect("midnight is valid")
            .and_utc());
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
        None => seven_days_ago
            .and_hms_opt(0, 0, 0)
            .expect("midnight is valid")
            .and_utc(),
    };

    let to = match params.to.as_deref() {
        Some(v) => parse_iso_date(v)?,
        None => today
            .and_hms_opt(23, 59, 59)
            .expect("23:59:59 is valid")
            .and_utc(),
    };

    let granularity = params.granularity.unwrap_or_default();

    let points = state
        .repo
        .usage_graph(
            &user.user_id,
            from,
            to,
            granularity,
            params.model.as_deref(),
        )
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query usage graph: {e}")))?;

    let total_points = state
        .repo
        .usage_graph_total(from, to, granularity, params.model.as_deref())
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("failed to query total usage: {e}")))?;

    let (user_inputs, user_outputs, user_cache) = points
        .iter()
        .fold((0i64, 0i64, 0i64), |(i, o, c), p| {
            (i + p.inputs, o + p.outputs, c + p.cache)
        });

    let (all_inputs, all_outputs, all_cache) = total_points
        .iter()
        .fold((0i64, 0i64, 0i64), |(i, o, c), p| {
            (i + p.inputs, o + p.outputs, c + p.cache)
        });

    let shared = model::usage_log::UsageShared {
        inputs: pct(user_inputs, all_inputs),
        outputs: pct(user_outputs, all_outputs),
        cache: pct(user_cache, all_cache),
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
