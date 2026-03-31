use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub upstream_api_key: String,
    pub table_name: String,
    pub keycloak_issuer: String,
    pub keycloak_jwks_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let upstream_api_key = env::var("UPSTREAM_API_KEY")
            .expect("UPSTREAM_API_KEY is required");

        let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string());

        let keycloak_issuer = env::var("KEYCLOAK_ISSUER").unwrap_or_default();
        let keycloak_jwks_url = env::var("KEYCLOAK_JWKS_URL")
            .unwrap_or_else(|_| format!("{keycloak_issuer}/protocol/openid-connect/certs"));

        Self {
            port,
            upstream_api_key,
            table_name,
            keycloak_issuer,
            keycloak_jwks_url,
        }
    }
}
