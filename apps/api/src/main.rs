mod auth;
mod config;
mod response;
mod routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use tracing_subscriber::EnvFilter;

use crate::auth::jwt::JwksCache;
use crate::config::Config;

pub struct AppInner {
    pub config: Config,
    pub repo: Box<dyn repository::Repository>,
    pub jwks_cache: JwksCache,
}

pub type AppState = Arc<AppInner>;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    run_server().await;
}

async fn run_server() {
    let cfg = Config::from_env();

    let repo = config::create_repo().await;

    let jwks_cache = JwksCache::new(cfg.keycloak_jwks_url.clone());

    let state: AppState = Arc::new(AppInner {
        config: cfg.clone(),
        repo: Box::new(repo),
        jwks_cache,
    });
    let port = state.config.port;

    let dashboard_routes = routes::dashboard::dashboard_router().layer(
        axum::middleware::from_fn_with_state(state.clone(), auth::require_jwt),
    );

    let app = Router::new()
        .nest("/dashboard", dashboard_routes)
        .route("/health", get(health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");

    tracing::info!("API listening on port {port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn health() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::{
            select,
            signal::unix::{SignalKind, signal},
        };
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
        select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv()  => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
