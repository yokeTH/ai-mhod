use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub upstream_api_key: String,
    pub allowed_api_keys: Vec<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let upstream_api_key = env::var("UPSTREAM_API_KEY")
            .expect("UPSTREAM_API_KEY is required");

        let allowed_api_keys: Vec<String> = env::var("ALLOWED_API_KEYS")
            .expect("ALLOWED_API_KEYS is required (comma-separated)")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            port,
            upstream_api_key,
            allowed_api_keys,
        }
    }
}
