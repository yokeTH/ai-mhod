use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub upstream_api_key: String,
    pub table_name: String,
    pub anthropic_client_id: String,
    pub anthropic_refresh_token: String,
    pub redis_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let upstream_api_key = env::var("UPSTREAM_API_KEY").expect("UPSTREAM_API_KEY is required");

        let table_name = dynamodb::DynamoDbRepo::table_name();

        let anthropic_client_id =
            env::var("ANTHROPIC_CLIENT_ID").expect("ANTHROPIC_CLIENT_ID is required");
        let anthropic_refresh_token =
            env::var("ANTHROPIC_REFRESH_TOKEN").expect("ANTHROPIC_REFRESH_TOKEN is required");
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self {
            port,
            upstream_api_key,
            table_name,
            anthropic_client_id,
            anthropic_refresh_token,
            redis_url,
        }
    }
}
