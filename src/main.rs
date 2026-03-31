use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use reqwest::Client;
use tracing_subscriber::EnvFilter;

use server::config;
use server::routes;
use server::{AppInner, AppState};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env();

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("failed to build HTTP client");

    let state: AppState = Arc::new(AppInner { client, config: cfg });
    let port = state.config.port;

    let app = Router::new()
        .nest("/zai", routes::zai_router())
        .route("/health", get(routes::health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");

    tracing::info!("Listening on port {port}");
    axum::serve(listener, app).await.expect("server error");
}
