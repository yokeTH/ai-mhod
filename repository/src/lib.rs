use model::usage_log::{UsageLog, UsageRow};
use model::user::{ApiKey, User};

#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    async fn create_user(&self, name: &str) -> anyhow::Result<i64>;
    async fn list_users(&self) -> anyhow::Result<Vec<User>>;
    async fn lookup_user_by_name(&self, name: &str) -> anyhow::Result<Option<i64>>;

    async fn create_key(&self, user_id: i64, name: Option<&str>) -> anyhow::Result<String>;
    async fn list_keys(&self, user_id: i64) -> anyhow::Result<Vec<ApiKey>>;
    async fn lookup_key(&self, key: &str) -> anyhow::Result<Option<i64>>;

    async fn insert_usage_log(&self, log: &UsageLog) -> anyhow::Result<()>;
    async fn usage_summary(&self, user_id: Option<i64>) -> anyhow::Result<Vec<UsageRow>>;
}
