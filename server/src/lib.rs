pub mod auth;
pub mod config;
pub mod error;
pub mod metrics;
pub mod proxy;
pub mod routes;

use std::sync::Arc;

use reqwest::Client;

use crate::config::Config;

pub struct AppInner {
    pub client: Client,
    pub config: Config,
}

pub type AppState = Arc<AppInner>;
