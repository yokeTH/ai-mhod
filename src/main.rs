use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use clap::Parser;
use reqwest::Client;
use repository::Repository;
use server::routes;
use server::{AppInner, AppState};
use sqlite::SqliteRepo;
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

fn open_db() -> SqliteRepo {
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "mhod.db".to_string());
    SqliteRepo::open(&db_path).expect("failed to open database")
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
        Commands::Usage { user } => run_usage(user).await,
    }
}

async fn run_server() {
    let cfg = server::config::Config::from_env();

    let repo = SqliteRepo::open(&cfg.db_path).expect("failed to open database");

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("failed to build HTTP client");

    let (usage_tx, mut usage_rx) = tokio::sync::mpsc::channel::<model::usage_log::UsageLog>(256);

    let writer_repo: SqliteRepo =
        SqliteRepo::open(&cfg.db_path).expect("failed to open database for writer");

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

    let zai_routes = routes::zai_router()
        .layer(axum::middleware::from_fn_with_state(state.clone(), server::auth::require_api_key));

    let app = Router::new()
        .nest("/zai", zai_routes)
        .route("/health", get(routes::health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind");

    tracing::info!("Listening on port {port}");
    axum::serve(listener, app).await.expect("server error");
}

async fn run_user_cmd(cmd: UserCmd) {
    let db = open_db();
    match cmd {
        UserCmd::Add { name } => {
            let id = db.create_user(&name).await.expect("failed to create user");
            println!("Created user '{name}' with id {id}");
        }
        UserCmd::List => {
            let users = db.list_users().await.expect("failed to list users");
            if users.is_empty() {
                println!("No users found.");
                return;
            }
            println!("{:<5} {:<20} {}", "ID", "Name", "Created");
            for u in users {
                println!("{:<5} {:<20} {}", u.id, u.name, u.created_at);
            }
        }
    }
}

async fn run_key_cmd(cmd: KeyCmd) {
    let db = open_db();
    match cmd {
        KeyCmd::Add { user, name } => {
            let user_id = db
                .lookup_user_by_name(&user)
                .await
                .expect("failed to look up user")
                .unwrap_or_else(|| panic!("user '{user}' not found"));
            let key = db
                .create_key(user_id, name.as_deref())
                .await
                .expect("failed to create key");
            println!("{key}");
        }
        KeyCmd::List { user } => {
            let user_id = db
                .lookup_user_by_name(&user)
                .await
                .expect("failed to look up user")
                .unwrap_or_else(|| panic!("user '{user}' not found"));
            let keys = db.list_keys(user_id).await.expect("failed to list keys");
            if keys.is_empty() {
                println!("No keys found for user '{user}'.");
                return;
            }
            println!("{:<5} {:<40} {:<15} {}", "ID", "Key", "Name", "Created");
            for k in keys {
                let name = k.name.unwrap_or_default();
                println!("{:<5} {:<40} {:<15} {}", k.id, k.key, name, k.created_at);
            }
        }
    }
}

async fn run_usage(user: Option<String>) {
    let db = open_db();
    let user_id = if let Some(ref name) = user {
        Some(
            db.lookup_user_by_name(name)
                .await
                .expect("failed to look up user")
                .unwrap_or_else(|| panic!("user '{name}' not found")),
        )
    } else {
        None
    };

    let rows = db
        .usage_summary(user_id)
        .await
        .expect("failed to get usage summary");

    if rows.is_empty() {
        println!("No usage data found.");
        return;
    }

    println!(
        "{:<15} {:<30} {:>8} {:>12} {:>12} {:>12} {:>12}",
        "User", "Model", "Requests", "Input Tok", "Output Tok", "Cache Read", "Duration ms"
    );
    for r in rows {
        println!(
            "{:<15} {:<30} {:>8} {:>12} {:>12} {:>12} {:>12}",
            r.user_name,
            r.model,
            r.total_requests,
            r.total_input_tokens,
            r.total_output_tokens,
            r.total_cache_read_tokens,
            r.total_duration_ms
        );
    }
}
