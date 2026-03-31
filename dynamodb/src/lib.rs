mod items;

use std::collections::HashMap;

use async_trait::async_trait;
use aws_sdk_dynamodb::types::{AttributeValue, Select};
use aws_sdk_dynamodb::Client;
use chrono::Utc;
use items::{KeyItem, UsageLogItem, UserItem};
use rand::Rng;
use repository::Repository;

pub struct DynamoDbRepo {
    client: Client,
    table_name: String,
}

impl DynamoDbRepo {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    fn generate_key() -> String {
        let hex: String = (0..32).map(|_| format!("{:02x}", rand::rng().random::<u8>())).collect();
        format!("mh_{hex}")
    }

    fn s(val: &str) -> AttributeValue {
        AttributeValue::S(val.to_string())
    }
}

#[async_trait]
impl Repository for DynamoDbRepo {
    async fn create_user(&self, name: &str) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let user = model::user::User {
            id: id.clone(),
            name: name.to_string(),
            created_at: now,
        };
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(UserItem::from(user))?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .condition_expression("attribute_not_exists(pk)")
            .send()
            .await?;

        Ok(id)
    }

    async fn list_users(&self) -> anyhow::Result<Vec<model::user::User>> {
        let mut users = Vec::new();
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut q = self
                .client
                .scan()
                .table_name(&self.table_name)
                .filter_expression("begins_with(pk, :prefix) AND #t = :type")
                .expression_attribute_names("#t", "type")
                .expression_attribute_values(":prefix", Self::s("USER#"))
                .expression_attribute_values(":type", Self::s("USER"));

            if let Some(key) = exclusive_start_key.take() {
                q = q.set_exclusive_start_key(Some(key));
            }

            let resp = q.send().await?;

            for item in resp.items() {
                let user_item: UserItem = serde_dynamo::from_item(item.clone())?;
                users.push(user_item.into());
            }

            if resp.last_evaluated_key().is_none() {
                break;
            }
            exclusive_start_key = resp.last_evaluated_key().cloned();
        }

        Ok(users)
    }

    async fn lookup_user_by_name(&self, name: &str) -> anyhow::Result<Option<String>> {
        let gsi1_pk = format!("USERNAME#{name}");

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("gsi1")
            .key_condition_expression("gsi1_pk = :pk")
            .expression_attribute_values(":pk", Self::s(&gsi1_pk))
            .select(Select::AllAttributes)
            .send()
            .await?;

        match resp.items().first() {
            Some(item) => {
                let user_item: UserItem = serde_dynamo::from_item(item.clone())?;
                Ok(Some(user_item.id))
            }
            None => Ok(None),
        }
    }

    async fn create_key(&self, user_id: &str, name: Option<&str>) -> anyhow::Result<(String, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        let key = Self::generate_key();
        let now = Utc::now().to_rfc3339();

        let api_key = model::user::ApiKey {
            id: id.clone(),
            user_id: user_id.to_string(),
            key: key.clone(),
            name: name.map(|s| s.to_string()),
            created_at: now,
            revoked: false,
        };
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(KeyItem::from(api_key))?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;

        Ok((id, key))
    }

    async fn list_keys(&self, user_id: &str) -> anyhow::Result<Vec<model::user::ApiKey>> {
        let gsi_pk = format!("USER#{user_id}");

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("gsi1")
            .key_condition_expression("gsi1_pk = :pk AND begins_with(gsi1_sk, :prefix)")
            .expression_attribute_values(":pk", Self::s(&gsi_pk))
            .expression_attribute_values(":prefix", Self::s("KEY#"))
            .select(Select::AllAttributes)
            .send()
            .await?;

        let mut keys = Vec::new();
        for item in resp.items() {
            let key_item: KeyItem = serde_dynamo::from_item(item.clone())?;
            keys.push(key_item.into());
        }

        Ok(keys)
    }

    async fn lookup_key(&self, key: &str) -> anyhow::Result<Option<(String, String, bool)>> {
        let gsi2_pk = format!("KEYVAL#{key}");

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("gsi2")
            .key_condition_expression("gsi2_pk = :pk")
            .expression_attribute_values(":pk", Self::s(&gsi2_pk))
            .select(Select::AllAttributes)
            .send()
            .await?;

        match resp.items().first() {
            Some(item) => {
                let key_item: KeyItem = serde_dynamo::from_item(item.clone())?;
                Ok(Some((key_item.user_id, key_item.id, key_item.revoked.unwrap_or(true))))
            }
            None => Ok(None),
        }
    }

    async fn revoke_key(&self, key_id: &str) -> anyhow::Result<()> {
        let pk = format!("KEY#{key_id}");

        self.client
            .update_item()
            .table_name(&self.table_name)
            .key("pk", Self::s(&pk))
            .key("sk", Self::s(&pk))
            .update_expression("SET revoked = :revoked")
            .expression_attribute_values(":revoked", AttributeValue::Bool(true))
            .condition_expression("attribute_exists(pk) AND #t = :key_type")
            .expression_attribute_names("#t", "type")
            .expression_attribute_values(":key_type", Self::s("KEY"))
            .send()
            .await?;

        Ok(())
    }

    async fn insert_usage_log(&self, log: &model::usage_log::UsageLog) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(UsageLogItem::from_log(log.clone(), now))?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;

        Ok(())
    }

    async fn usage_summary(&self, user_id: Option<&str>, api_key_id: Option<&str>) -> anyhow::Result<Vec<model::usage_log::UsageRow>> {
        let mut aggregates: HashMap<(String, String, Option<String>), model::usage_log::UsageRow> = HashMap::new();

        let pk_values: Vec<String> = match user_id {
            Some(id) => vec![format!("USER#{id}")],
            None => {
                let users = self.list_users().await?;
                users.iter().map(|u| format!("USER#{}", u.id)).collect()
            }
        };

        for pk in &pk_values {
            let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

            loop {
                let mut q = self
                    .client
                    .query()
                    .table_name(&self.table_name)
                    .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
                    .expression_attribute_values(":pk", Self::s(pk))
                    .expression_attribute_values(":prefix", Self::s("LOG#"));

                if let Some(key) = exclusive_start_key.take() {
                    q = q.set_exclusive_start_key(Some(key));
                }

                let resp = q.send().await?;

                for item in resp.items() {
                    let log_item: UsageLogItem = serde_dynamo::from_item(item.clone())?;

                    let u_id = pk.strip_prefix("USER#").unwrap_or_default().to_string();

                    if let Some(filter_key_id) = api_key_id
                        && log_item.api_key_id != filter_key_id
                    {
                        continue;
                    }

                    let key = (u_id.clone(), log_item.model.clone(), Some(log_item.api_key_id.clone()));

                    let entry = aggregates.entry(key).or_insert_with(|| model::usage_log::UsageRow {
                        user_id: u_id,
                        model: log_item.model.clone(),
                        api_key_id: Some(log_item.api_key_id.clone()),
                        total_requests: 0,
                        total_input_tokens: 0,
                        total_output_tokens: 0,
                        total_cache_read_tokens: 0,
                        total_duration_ms: 0,
                    });

                    entry.total_requests += 1;
                    entry.total_input_tokens += log_item.input_tokens.unwrap_or(0) as i64;
                    entry.total_output_tokens += log_item.output_tokens.unwrap_or(0) as i64;
                    entry.total_cache_read_tokens += log_item.cache_read_tokens.unwrap_or(0) as i64;
                    entry.total_duration_ms += log_item.duration_ms as i64;
                }

                if resp.last_evaluated_key().is_none() {
                    break;
                }
                exclusive_start_key = resp.last_evaluated_key().cloned();
            }
        }

        let mut rows: Vec<_> = aggregates.into_values().collect();
        rows.sort_by(|a, b| (&a.user_id, &a.model).cmp(&(&b.user_id, &b.model)));
        Ok(rows)
    }
}
