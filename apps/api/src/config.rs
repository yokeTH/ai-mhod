use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub keycloak_issuer: String,
    pub keycloak_jwks_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let keycloak_issuer = env::var("KEYCLOAK_ISSUER").unwrap_or_default();
        let keycloak_jwks_url = env::var("KEYCLOAK_JWKS_URL")
            .unwrap_or_else(|_| format!("{keycloak_issuer}/protocol/openid-connect/certs"));

        Self {
            port,
            keycloak_issuer,
            keycloak_jwks_url,
        }
    }
}
