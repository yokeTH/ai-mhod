use aws_config::BehaviorVersion;
use clap::Parser;
use dynamodb::DynamoDbRepo;
use repository::Repository;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "cli", version, about = "AI API proxy admin CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
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
    /// Set the Keycloak subject for a user
    SetKeycloak {
        /// User name
        user: String,
        /// Keycloak subject (sub claim)
        sub: String,
    },
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
    /// Revoke an API key
    Revoke {
        /// Key ID
        key_id: String,
    },
}

async fn create_repo() -> DynamoDbRepo {
    let table_name = std::env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string());
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);
    DynamoDbRepo::new(client, table_name)
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
        Commands::User { cmd } => run_user_cmd(cmd).await,
        Commands::Key { cmd } => run_key_cmd(cmd).await,
        Commands::Usage { user, key } => run_usage(user, key).await,
    }
}

async fn run_user_cmd(cmd: UserCmd) {
    let db = create_repo().await;
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
        UserCmd::SetKeycloak { user, sub } => {
            let user_id = db
                .lookup_user_by_name(&user)
                .await
                .expect("failed to look up user");
            if user_id.is_none() {
                eprintln!("user '{user}' not found");
                std::process::exit(1);
            }
            db.update_keycloak_sub(&user_id.unwrap(), &sub)
                .await
                .expect("failed to update keycloak sub");
            println!("Updated keycloak_sub for '{user}' to '{sub}'");
        }
    }
}

async fn run_key_cmd(cmd: KeyCmd) {
    let db = create_repo().await;
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
            println!("{:<40} {:<15} {:<8} Created", "Key", "Name", "Revoked");
            for k in keys {
                let name = k.name.unwrap_or_default();
                let revoked = if k.revoked { "yes" } else { "no" };
                println!("{:<40} {:<15} {:<8} {}", k.key, name, revoked, k.created_at);
            }
        }
        KeyCmd::Revoke { key_id } => {
            db.revoke_key(&key_id).await.expect("failed to revoke key");
            println!("Key '{key_id}' revoked.");
        }
    }
}

async fn run_usage(user: Option<String>, key: Option<String>) {
    let db = create_repo().await;

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
