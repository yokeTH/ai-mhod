pub mod dto;
mod auth;
mod config;
mod metrics;
mod proxy;
mod routes;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use repository::Repository;
use reqwest::Client;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

pub type UsageTx = tokio::sync::mpsc::Sender<model::usage_log::UsageLog>;

pub struct AppInner {
    pub client: Client,
    pub config: Config,
    pub repo: Box<dyn repository::Repository>,
    pub usage_tx: UsageTx,
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

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("failed to build HTTP client");

    let (usage_tx, mut usage_rx) = tokio::sync::mpsc::channel::<model::usage_log::UsageLog>(256);

    let writer_repo = config::create_repo().await;

    tokio::spawn(async move {
        while let Some(log) = usage_rx.recv().await {
            if let Err(e) = writer_repo.insert_usage_log(&log).await {
                tracing::error!(error = %e, request_id = %log.request_id, "Failed to insert usage log");
            }
        }
    });

    let state: AppState = Arc::new(AppInner {
        client,
        config: cfg.clone(),
        repo: Box::new(repo),
        usage_tx,
    });
    let port = state.config.port;

    let zai_routes = routes::zai::zai_router().layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::require_api_key,
    ));

    let app = Router::new()
        .nest("/zai", zai_routes)
        .route("/health", get(routes::zai::health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");

    tracing::info!("Proxy listening on port {port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
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
