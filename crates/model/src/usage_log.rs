use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLog {
    pub request_id: String,
    pub user_id: String,
    pub api_key_id: String,
    pub model: String,
    pub stream: bool,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRow {
    pub user_id: String,
    pub model: String,
    pub api_key_id: Option<String>,
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageGraphPoint {
    pub period: String,
    pub inputs: i64,
    pub outputs: i64,
    pub cache: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageShared {
    pub inputs: f64,
    pub outputs: f64,
    pub cache: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageGraphResponse {
    pub points: Vec<UsageGraphPoint>,
    pub shared: UsageShared,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Granularity {
    #[serde(rename = "15min")]
    FifteenMin,
    #[serde(rename = "30min")]
    ThirtyMin,
    #[serde(rename = "1hr")]
    #[default]
    OneHour,
    #[serde(rename = "4hr")]
    FourHours,
    #[serde(rename = "12hr")]
    TwelveHours,
    Daily,
    Weekly,
    Monthly,
}
