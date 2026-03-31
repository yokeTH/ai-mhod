use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rand::Rng;
use repository::Repository;
use rusqlite::{params, Row};
use tokio::task::spawn_blocking;

pub struct SqliteRepo {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS users (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS api_keys (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key        TEXT NOT NULL UNIQUE,
    name       TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS usage_logs (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id          TEXT NOT NULL,
    user_id             INTEGER NOT NULL REFERENCES users(id),
    model               TEXT NOT NULL,
    stream              BOOLEAN NOT NULL,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cache_read_tokens   INTEGER,
    duration_ms         INTEGER NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);
";

impl SqliteRepo {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn generate_key() -> String {
        let hex: String = (0..32).map(|_| format!("{:02x}", rand::rng().random::<u8>())).collect();
        format!("mh_{hex}")
    }

    fn row_to_user(row: &Row<'_>) -> rusqlite::Result<model::user::User> {
        Ok(model::user::User {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
        })
    }

    fn row_to_api_key(row: &Row<'_>) -> rusqlite::Result<model::user::ApiKey> {
        Ok(model::user::ApiKey {
            id: row.get(0)?,
            user_id: row.get(1)?,
            key: row.get(2)?,
            name: row.get(3)?,
            created_at: row.get(4)?,
        })
    }

    fn row_to_usage_row(row: &Row<'_>) -> rusqlite::Result<model::usage_log::UsageRow> {
        Ok(model::usage_log::UsageRow {
            user_name: row.get(0)?,
            model: row.get(1)?,
            total_requests: row.get(2)?,
            total_input_tokens: row.get(3)?,
            total_output_tokens: row.get(4)?,
            total_cache_read_tokens: row.get(5)?,
            total_duration_ms: row.get(6)?,
        })
    }
}

#[async_trait]
impl Repository for SqliteRepo {
    async fn create_user(&self, name: &str) -> anyhow::Result<i64> {
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute("INSERT INTO users (name) VALUES (?1)", params![name])?;
            Ok(conn.last_insert_rowid())
        })
        .await?
    }

    async fn list_users(&self) -> anyhow::Result<Vec<model::user::User>> {
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT id, name, created_at FROM users ORDER BY id")?;
            let rows = stmt
                .query_map([], Self::row_to_user)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn lookup_user_by_name(&self, name: &str) -> anyhow::Result<Option<i64>> {
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id FROM users WHERE name = ?1")?;
            let mut rows = stmt.query(params![name])?;
            match rows.next()? {
                Some(row) => Ok(Some(row.get(0)?)),
                None => Ok(None),
            }
        })
        .await?
    }

    async fn create_key(&self, user_id: i64, name: Option<&str>) -> anyhow::Result<String> {
        let key = Self::generate_key();
        let name = name.map(String::from);
        let conn = Arc::clone(&self.conn);
        let key_clone = key.clone();
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO api_keys (user_id, key, name) VALUES (?1, ?2, ?3)",
                params![user_id, key_clone, name],
            )?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(key)
    }

    async fn list_keys(&self, user_id: i64) -> anyhow::Result<Vec<model::user::ApiKey>> {
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, user_id, key, name, created_at FROM api_keys WHERE user_id = ?1 ORDER BY id",
            )?;
            let rows = stmt
                .query_map(params![user_id], Self::row_to_api_key)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }

    async fn lookup_key(&self, key: &str) -> anyhow::Result<Option<i64>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT user_id FROM api_keys WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            match rows.next()? {
                Some(row) => Ok(Some(row.get(0)?)),
                None => Ok(None),
            }
        })
        .await?
    }

    async fn insert_usage_log(&self, log: &model::usage_log::UsageLog) -> anyhow::Result<()> {
        let log = log.clone();
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO usage_logs (request_id, user_id, model, stream, input_tokens, output_tokens, cache_read_tokens, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    log.request_id,
                    log.user_id,
                    log.model,
                    log.stream,
                    log.input_tokens,
                    log.output_tokens,
                    log.cache_read_tokens,
                    log.duration_ms,
                ],
            )?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    async fn usage_summary(&self, user_id: Option<i64>) -> anyhow::Result<Vec<model::usage_log::UsageRow>> {
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = if let Some(uid) = user_id {
                let mut stmt = conn.prepare(
                    "SELECT u.name, ul.model, COUNT(*) as total_requests,
                            COALESCE(SUM(ul.input_tokens), 0) as total_input_tokens,
                            COALESCE(SUM(ul.output_tokens), 0) as total_output_tokens,
                            COALESCE(SUM(ul.cache_read_tokens), 0) as total_cache_read_tokens,
                            COALESCE(SUM(ul.duration_ms), 0) as total_duration_ms
                     FROM usage_logs ul JOIN users u ON u.id = ul.user_id
                     WHERE ul.user_id = ?1
                     GROUP BY ul.model
                     ORDER BY ul.model",
                )?;
                stmt.query_map(params![uid], Self::row_to_usage_row)?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                let mut stmt = conn.prepare(
                    "SELECT u.name, ul.model, COUNT(*) as total_requests,
                            COALESCE(SUM(ul.input_tokens), 0) as total_input_tokens,
                            COALESCE(SUM(ul.output_tokens), 0) as total_output_tokens,
                            COALESCE(SUM(ul.cache_read_tokens), 0) as total_cache_read_tokens,
                            COALESCE(SUM(ul.duration_ms), 0) as total_duration_ms
                     FROM usage_logs ul JOIN users u ON u.id = ul.user_id
                     GROUP BY u.name, ul.model
                     ORDER BY u.name, ul.model",
                )?;
                stmt.query_map([], Self::row_to_usage_row)?
                    .collect::<Result<Vec<_>, _>>()?
            };
            Ok(rows)
        })
        .await?
    }
}
