pub mod auth;
pub mod config;
pub mod error;
pub mod metrics;
pub mod proxy;
pub mod response;
pub mod routes;

use std::sync::Arc;

use reqwest::Client;

use crate::auth::jwt::JwksCache;
use crate::config::Config;

pub type UsageTx = tokio::sync::mpsc::Sender<model::usage_log::UsageLog>;

pub struct AppInner {
    pub client: Client,
    pub config: Config,
    pub repo: Box<dyn repository::Repository>,
    pub usage_tx: UsageTx,
    pub jwks_cache: JwksCache,
}

pub type AppState = Arc<AppInner>;
