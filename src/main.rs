mod config;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clap::Parser;
use repository::Repository;
use reqwest::Client;
use server::routes;
use server::{AppInner, AppState};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "mhod", version, about = "AI API proxy with usage tracking")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the proxy server
    Serve,
    /// Manage users
    User {
        #[command(subcommand)]
        cmd: UserCmd,
    },
    /// Manage API keys
    Key {
        #[command(subcommand)]
        cmd: KeyCmd,
    },
    /// Show usage summary
    Usage {
        /// Filter by user name
        user: Option<String>,
        /// Filter by API key id
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(clap::Subcommand)]
enum UserCmd {
    /// Create a new user
    Add { name: String },
    /// List all users
    List,
}

#[derive(clap::Subcommand)]
enum KeyCmd {
    /// Create a new API key for a user
    Add {
        /// User name
        user: String,
        /// Optional key name
        #[arg(long)]
        name: Option<String>,
    },
    /// List API keys for a user
    List {
        /// User name
        user: String,
    },
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve => run_server().await,
        Commands::User { cmd } => run_user_cmd(cmd).await,
        Commands::Key { cmd } => run_key_cmd(cmd).await,
        Commands::Usage { user, key } => run_usage(user, key).await,
    }
}

async fn run_server() {
    let cfg = server::config::Config::from_env();

    let repo = config::create_repo().await;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("failed to build HTTP client");

    let (usage_tx, mut usage_rx) = tokio::sync::mpsc::channel::<model::usage_log::UsageLog>(256);

    let writer_repo = config::create_repo().await;

    // Spawn a background task to write usage logs
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

    let zai_routes = routes::zai_router().layer(axum::middleware::from_fn_with_state(
        state.clone(),
        server::auth::require_api_key,
    ));

    let app = Router::new()
        .nest("/zai", zai_routes)
        .route("/health", get(routes::health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");

    tracing::info!("Listening on port {port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn run_user_cmd(cmd: UserCmd) {
    let db = config::create_repo().await;
    match cmd {
        UserCmd::Add { name } => {
            let id = db.create_user(&name).await.expect("failed to create user");
            println!("Created user '{name}' (id: {id})");
        }
        UserCmd::List => {
            let users = db.list_users().await.expect("failed to list users");
            if users.is_empty() {
                println!("No users found.");
                return;
            }
            println!("{:<40} {:<20} Created", "ID", "Name");
            for u in users {
                println!("{:<40} {:<20} {}", u.id, u.name, u.created_at);
            }
        }
    }
}

async fn run_key_cmd(cmd: KeyCmd) {
    let db = config::create_repo().await;
    match cmd {
        KeyCmd::Add { user, name } => {
            let user_id = db
                .lookup_user_by_name(&user)
                .await
                .expect("failed to look up user");
            if user_id.is_none() {
                eprintln!("user '{user}' not found");
                std::process::exit(1);
            }
            let user_id = user_id.unwrap();
            let (_id, key) = db
                .create_key(&user_id, name.as_deref())
                .await
                .expect("failed to create key");
            println!("{key}");
        }
        KeyCmd::List { user } => {
            let user_id = db
                .lookup_user_by_name(&user)
                .await
                .expect("failed to look up user");
            if user_id.is_none() {
                eprintln!("user '{user}' not found");
                std::process::exit(1);
            }
            let keys = db
                .list_keys(&user_id.unwrap())
                .await
                .expect("failed to list keys");
            if keys.is_empty() {
                println!("No keys found for user '{user}'.");
                return;
            }
            println!("{:<40} {:<15} Created", "Key", "Name");
            for k in keys {
                let name = k.name.unwrap_or_default();
                println!("{:<40} {:<15} {}", k.key, name, k.created_at);
            }
        }
    }
}

async fn run_usage(user: Option<String>, key: Option<String>) {
    let db = config::create_repo().await;

    let rows = db
        .usage_summary(user.as_deref(), key.as_deref())
        .await
        .expect("failed to get usage summary");

    if rows.is_empty() {
        println!("No usage data found.");
        return;
    }

    println!(
        "{:<15} {:<30} {:<40} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "User",
        "Model",
        "API Key ID",
        "Requests",
        "Input Tok",
        "Output Tok",
        "Cache Read",
        "Duration ms"
    );
    for r in rows {
        let api_key_id = r.api_key_id.unwrap_or_else(|| "-".to_string());
        println!(
            "{:<15} {:<30} {:<40} {:>8} {:>12} {:>12} {:>12} {:>12}",
            r.user_id,
            r.model,
            api_key_id,
            r.total_requests,
            r.total_input_tokens,
            r.total_output_tokens,
            r.total_cache_read_tokens,
            r.total_duration_ms
        );
    }
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
