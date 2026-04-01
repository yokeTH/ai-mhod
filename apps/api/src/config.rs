use std::env;

use aws_config::BehaviorVersion;
use dynamodb::DynamoDbRepo;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
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

        let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string());

        let keycloak_issuer = env::var("KEYCLOAK_ISSUER").unwrap_or_default();
        let keycloak_jwks_url = env::var("KEYCLOAK_JWKS_URL")
            .unwrap_or_else(|_| format!("{keycloak_issuer}/protocol/openid-connect/certs"));

        Self {
            port,
            table_name,
            keycloak_issuer,
            keycloak_jwks_url,
        }
    }
}

pub fn table_name() -> String {
    std::env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string())
}

pub async fn create_repo() -> DynamoDbRepo {
    let table_name = table_name();
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);
    DynamoDbRepo::new(client, table_name)
}
