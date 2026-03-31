use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogItem {
    pub pk: String,
    pub sk: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub user_id: String,
    pub api_key_id: String,
    pub request_id: String,
    pub model: String,
    pub stream: bool,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub duration_ms: u64,
    pub created_at: String,
}

impl UsageLogItem {
    pub fn from_log(log: model::usage_log::UsageLog, created_at: String) -> Self {
        let pk = format!("USER#{}", log.user_id);
        let sk = format!("LOG#{created_at}#{}", log.request_id);
        Self {
            pk,
            sk,
            item_type: "LOG".to_string(),
            user_id: log.user_id,
            api_key_id: log.api_key_id,
            request_id: log.request_id,
            model: log.model,
            stream: log.stream,
            input_tokens: log.input_tokens,
            output_tokens: log.output_tokens,
            cache_read_tokens: log.cache_read_tokens,
            duration_ms: log.duration_ms,
            created_at,
        }
    }
}
