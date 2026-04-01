use model::usage_log::{Granularity, UsageLog, UsageRow};
use model::user::{ApiKey, User};

#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    async fn create_user(&self, name: &str) -> anyhow::Result<String>;
    async fn list_users(&self) -> anyhow::Result<Vec<User>>;
    async fn lookup_user_by_name(&self, name: &str) -> anyhow::Result<Option<String>>;

    async fn create_key(
        &self,
        user_id: &str,
        name: Option<&str>,
    ) -> anyhow::Result<(String, String)>;
    async fn list_keys(&self, user_id: &str) -> anyhow::Result<Vec<ApiKey>>;
    async fn lookup_key(&self, key: &str) -> anyhow::Result<Option<(String, String, bool)>>;
    async fn revoke_key(&self, key_id: &str) -> anyhow::Result<()>;

    async fn insert_usage_log(&self, log: &UsageLog) -> anyhow::Result<()>;
    async fn usage_summary(
        &self,
        user_id: Option<&str>,
        api_key_id: Option<&str>,
    ) -> anyhow::Result<Vec<UsageRow>>;

    async fn update_keycloak_sub(&self, user_id: &str, sub: &str) -> anyhow::Result<()>;

    async fn lookup_user_by_keycloak_sub(&self, sub: &str) -> anyhow::Result<Option<String>>;
    async fn usage_graph(
        &self,
        user_id: &str,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        granularity: Granularity,
        model_filter: Option<&str>,
    ) -> anyhow::Result<Vec<model::usage_log::UsageGraphPoint>>;

    async fn usage_graph_total(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        granularity: Granularity,
        model_filter: Option<&str>,
    ) -> anyhow::Result<Vec<model::usage_log::UsageGraphPoint>>;

    async fn list_models(&self, user_id: &str) -> anyhow::Result<Vec<String>>;
}
