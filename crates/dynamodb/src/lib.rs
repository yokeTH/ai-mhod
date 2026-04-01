mod items;

use std::collections::HashMap;
use std::fmt::Write;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::{AttributeValue, Select};
use chrono::{Datelike, Duration, Timelike, Utc};
use items::{KeyItem, UsageLogItem, UserItem};
use rand::Rng;
use repository::Repository;

#[derive(Clone)]
pub struct DynamoDbRepo {
    client: Client,
    table_name: String,
}

impl DynamoDbRepo {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub fn table_name() -> String {
        std::env::var("TABLE_NAME").unwrap_or_else(|_| "mhod".to_string())
    }

    pub async fn from_env() -> Self {
        let table_name = Self::table_name();
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = aws_sdk_dynamodb::Client::new(&config);
        Self::new(client, table_name)
    }

    #[must_use]
    fn generate_key() -> String {
        let mut buf = String::with_capacity(65);
        buf.push_str("mh_");
        for _ in 0..32 {
            write!(buf, "{:02x}", rand::rng().random::<u8>())
                .expect("writing to String never fails");
        }
        buf
    }

    #[inline]
    fn s(val: &str) -> AttributeValue {
        AttributeValue::S(val.to_string())
    }

    /// Query usage logs for a single user within a time range, optionally filtered by model.
    async fn query_usage_logs(
        &self,
        user_id: &str,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        model_filter: Option<&str>,
    ) -> anyhow::Result<Vec<UsageLogItem>> {
        let mut logs = Vec::new();
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

        let (pk_val, index, pk_name, from_sk, to_sk) = match model_filter {
            Some(model) => (
                format!("USERMODEL#{user_id}#{model}"),
                Some("gsi1".to_string()),
                "gsi1_pk",
                format!("LOG#{}", from.to_rfc3339()),
                format!("LOG#{}", to.to_rfc3339()),
            ),
            None => (
                format!("USER#{user_id}"),
                None,
                "pk",
                format!("LOG#{}", from.to_rfc3339()),
                format!("LOG#{}", to.to_rfc3339()),
            ),
        };

        let sk_name = match index {
            Some(_) => "gsi1_sk",
            None => "sk",
        };

        let key_expr =
            format!("{pk_name} = :pk AND {sk_name} BETWEEN :from_sk AND :to_sk");

        loop {
            let mut q = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression(&key_expr)
                .expression_attribute_values(":pk", Self::s(&pk_val))
                .expression_attribute_values(":from_sk", Self::s(&from_sk))
                .expression_attribute_values(":to_sk", Self::s(&to_sk));

            if let Some(ref idx) = index {
                q = q.index_name(idx);
            }

            if let Some(key) = exclusive_start_key.take() {
                q = q.set_exclusive_start_key(Some(key));
            }

            let resp = q.send().await?;

            for item in resp.items() {
                logs.push(serde_dynamo::from_item(item.clone())?);
            }

            if resp.last_evaluated_key().is_none() {
                break;
            }
            exclusive_start_key = resp.last_evaluated_key().cloned();
        }

        Ok(logs)
    }
}

#[must_use]
fn align_down(
    dt: chrono::DateTime<chrono::Utc>,
    granularity: model::usage_log::Granularity,
) -> chrono::DateTime<chrono::Utc> {
    match granularity {
        model::usage_log::Granularity::FifteenMin => {
            let slot = (dt.minute() / 15) * 15;
            dt.with_second(0)
                .expect("align_down: 0 is valid second")
                .with_minute(slot)
                .expect("align_down: slot is valid minute")
        }
        model::usage_log::Granularity::ThirtyMin => {
            let slot = (dt.minute() / 30) * 30;
            dt.with_second(0)
                .expect("align_down: 0 is valid second")
                .with_minute(slot)
                .expect("align_down: slot is valid minute")
        }
        model::usage_log::Granularity::OneHour => dt
            .with_second(0)
            .expect("align_down: 0 is valid second")
            .with_minute(0)
            .expect("align_down: 0 is valid minute"),
        model::usage_log::Granularity::FourHours => {
            let slot = (dt.hour() / 4) * 4;
            dt.with_second(0)
                .expect("align_down: 0 is valid second")
                .with_minute(0)
                .expect("align_down: 0 is valid minute")
                .with_hour(slot)
                .expect("align_down: slot is valid hour")
        }
        model::usage_log::Granularity::TwelveHours => {
            let slot = (dt.hour() / 12) * 12;
            dt.with_second(0)
                .expect("align_down: 0 is valid second")
                .with_minute(0)
                .expect("align_down: 0 is valid minute")
                .with_hour(slot)
                .expect("align_down: slot is valid hour")
        }
        model::usage_log::Granularity::Daily => dt
            .with_second(0)
            .expect("align_down: 0 is valid second")
            .with_minute(0)
            .expect("align_down: 0 is valid minute")
            .with_hour(0)
            .expect("align_down: 0 is valid hour"),
        model::usage_log::Granularity::Weekly => {
            let weekday = dt.weekday().num_days_from_monday();
            dt.with_second(0)
                .expect("align_down: 0 is valid second")
                .with_minute(0)
                .expect("align_down: 0 is valid minute")
                .with_hour(0)
                .expect("align_down: 0 is valid hour")
                - Duration::days(weekday as i64)
        }
        model::usage_log::Granularity::Monthly => dt
            .with_second(0)
            .expect("align_down: 0 is valid second")
            .with_minute(0)
            .expect("align_down: 0 is valid minute")
            .with_hour(0)
            .expect("align_down: 0 is valid hour")
            .with_day(1)
            .expect("align_down: 1 is valid day"),
    }
}

fn step_period(
    dt: chrono::DateTime<chrono::Utc>,
    granularity: model::usage_log::Granularity,
) -> Option<chrono::DateTime<chrono::Utc>> {
    use model::usage_log::Granularity;
    match granularity {
        Granularity::FifteenMin => dt.checked_add_signed(Duration::minutes(15)),
        Granularity::ThirtyMin => dt.checked_add_signed(Duration::minutes(30)),
        Granularity::OneHour => dt.checked_add_signed(Duration::hours(1)),
        Granularity::FourHours => dt.checked_add_signed(Duration::hours(4)),
        Granularity::TwelveHours => dt.checked_add_signed(Duration::hours(12)),
        Granularity::Daily => dt.checked_add_signed(Duration::days(1)),
        Granularity::Weekly => dt.checked_add_signed(Duration::weeks(1)),
        Granularity::Monthly => {
            let (new_year, new_month) = if dt.month() == 12 {
                (dt.year() + 1, 1u32)
            } else {
                (dt.year(), dt.month() + 1)
            };
            dt.with_year(new_year)?.with_month(new_month)
        }
    }
}

#[must_use]
fn generate_periods(
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
    granularity: model::usage_log::Granularity,
) -> Vec<String> {
    let mut periods = Vec::new();
    let mut current = align_down(from, granularity);
    while current <= to {
        periods.push(current.format("%Y-%m-%dT%H:%M:%SZ").to_string());
        current = match step_period(current, granularity) {
            Some(next) => next,
            None => break,
        };
    }
    periods
}

#[must_use]
fn aggregate_to_buckets(
    logs: &[UsageLogItem],
    granularity: model::usage_log::Granularity,
) -> std::collections::BTreeMap<String, (i64, i64, i64)> {
    let mut buckets: std::collections::BTreeMap<String, (i64, i64, i64)> =
        std::collections::BTreeMap::new();
    merge_into_buckets(logs, granularity, &mut buckets);
    buckets
}

fn merge_into_buckets(
    logs: &[UsageLogItem],
    granularity: model::usage_log::Granularity,
    buckets: &mut std::collections::BTreeMap<String, (i64, i64, i64)>,
) {
    for log_item in logs {
        let Some(created) = chrono::DateTime::parse_from_rfc3339(&log_item.created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok()
        else {
            continue;
        };

        let aligned = align_down(created, granularity);
        let period_key = aligned.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let entry = buckets.entry(period_key).or_insert((0, 0, 0));
        entry.0 += log_item.input_tokens.unwrap_or(0) as i64;
        entry.1 += log_item.output_tokens.unwrap_or(0) as i64;
        entry.2 += log_item.cache_read_tokens.unwrap_or(0) as i64;
    }
}

#[must_use]
fn fill_periods(
    buckets: std::collections::BTreeMap<String, (i64, i64, i64)>,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
    granularity: model::usage_log::Granularity,
) -> Vec<model::usage_log::UsageGraphPoint> {
    let periods = generate_periods(from, to, granularity);
    periods
        .into_iter()
        .map(|period| {
            let (inputs, outputs, cache) = buckets.get(&period).copied().unwrap_or((0, 0, 0));
            model::usage_log::UsageGraphPoint {
                period,
                inputs,
                outputs,
                cache,
            }
        })
        .collect()
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
            keycloak_sub: None,
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

    async fn create_key(
        &self,
        user_id: &str,
        name: Option<&str>,
    ) -> anyhow::Result<repository::CreatedKey> {
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

        Ok(repository::CreatedKey { id, key })
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

    async fn lookup_key(&self, key: &str) -> anyhow::Result<Option<repository::KeyLookup>> {
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
                Ok(Some(repository::KeyLookup {
                    user_id: key_item.user_id,
                    api_key_id: key_item.id,
                    revoked: key_item.revoked.unwrap_or(true),
                }))
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

    async fn insert_usage_log(&self, log: model::usage_log::UsageLog) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(UsageLogItem::from_log(log, now))?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;

        Ok(())
    }

    async fn usage_summary(
        &self,
        user_id: Option<&str>,
        api_key_id: Option<&str>,
    ) -> anyhow::Result<Vec<model::usage_log::UsageRow>> {
        let mut aggregates: HashMap<(String, String, Option<String>), model::usage_log::UsageRow> =
            HashMap::new();

        let pk_values: Vec<String> = match user_id {
            Some(id) => vec![format!("USER#{id}")],
            None => {
                let users = self.list_users().await?;
                users.iter().map(|u| format!("USER#{}", u.id)).collect()
            }
        };

        for pk in &pk_values {
            let u_id = pk.strip_prefix("USER#").unwrap_or_default().to_string();
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

                    if let Some(filter_key_id) = api_key_id
                        && log_item.api_key_id != filter_key_id
                    {
                        continue;
                    }

                    let key = (
                        u_id.clone(),
                        log_item.model.clone(),
                        Some(log_item.api_key_id.clone()),
                    );

                    let entry =
                        aggregates
                            .entry(key)
                            .or_insert_with(|| model::usage_log::UsageRow {
                                user_id: u_id.clone(),
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

    async fn update_keycloak_sub(&self, user_id: &str, sub: &str) -> anyhow::Result<()> {
        let pk = format!("USER#{user_id}");
        let gsi3_pk = format!("KC#{sub}");

        self.client
            .update_item()
            .table_name(&self.table_name)
            .key("pk", Self::s(&pk))
            .key("sk", Self::s(&pk))
            .update_expression("SET keycloak_sub = :sub, gsi3_pk = :gsi3pk, gsi3_sk = :gsi3sk")
            .expression_attribute_values(":sub", Self::s(sub))
            .expression_attribute_values(":gsi3pk", Self::s(&gsi3_pk))
            .expression_attribute_values(":gsi3sk", Self::s(&gsi3_pk))
            .condition_expression("attribute_exists(pk) AND #t = :type")
            .expression_attribute_names("#t", "type")
            .expression_attribute_values(":type", Self::s("USER"))
            .send()
            .await?;

        Ok(())
    }

    async fn lookup_user_by_keycloak_sub(&self, sub: &str) -> anyhow::Result<Option<String>> {
        let gsi3_pk = format!("KC#{sub}");

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("gsi3")
            .key_condition_expression("gsi3_pk = :pk")
            .expression_attribute_values(":pk", Self::s(&gsi3_pk))
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

    async fn usage_graph(
        &self,
        user_id: &str,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        granularity: model::usage_log::Granularity,
        model_filter: Option<&str>,
    ) -> anyhow::Result<Vec<model::usage_log::UsageGraphPoint>> {
        let logs = self
            .query_usage_logs(user_id, from, to, model_filter)
            .await?;
        let buckets = aggregate_to_buckets(&logs, granularity);
        Ok(fill_periods(buckets, from, to, granularity))
    }

    async fn usage_graph_total(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        granularity: model::usage_log::Granularity,
        model_filter: Option<&str>,
    ) -> anyhow::Result<Vec<model::usage_log::UsageGraphPoint>> {
        let users = self.list_users().await?;
        let model_filter_owned = model_filter.map(|s| s.to_string());

        let mut join_set = tokio::task::JoinSet::new();
        for user in users {
            let repo = self.clone();
            let filter = model_filter_owned.clone();
            join_set.spawn(async move {
                repo.query_usage_logs(&user.id, from, to, filter.as_deref())
                    .await
            });
        }

        let mut buckets: std::collections::BTreeMap<String, (i64, i64, i64)> =
            std::collections::BTreeMap::new();
        while let Some(result) = join_set.join_next().await {
            let logs = result??;
            merge_into_buckets(&logs, granularity, &mut buckets);
        }

        Ok(fill_periods(buckets, from, to, granularity))
    }

    async fn list_models(&self, user_id: &str) -> anyhow::Result<Vec<String>> {
        let mut models = std::collections::BTreeSet::new();
        let pk = format!("USER#{user_id}");
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut q = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
                .expression_attribute_values(":pk", Self::s(&pk))
                .expression_attribute_values(":prefix", Self::s("LOG#"))
                .projection_expression("#m");

            q = q.expression_attribute_names("#m", "model");

            if let Some(key) = exclusive_start_key.take() {
                q = q.set_exclusive_start_key(Some(key));
            }

            let resp = q.send().await?;

            for item in resp.items() {
                if let Some(AttributeValue::S(m)) = item.get("model") {
                    models.insert(m.clone());
                }
            }

            if resp.last_evaluated_key().is_none() {
                break;
            }
            exclusive_start_key = resp.last_evaluated_key().cloned();
        }

        Ok(models.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::usage_log::Granularity;

    fn dt(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .expect("invalid test datetime")
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn align_down_fifteen_min_rounds_to_slot() {
        let input = dt("2026-04-02T14:37:22Z");
        let result = align_down(input, Granularity::FifteenMin);
        assert_eq!(result, dt("2026-04-02T14:30:00Z"));
    }

    #[test]
    fn align_down_fifteen_min_on_boundary() {
        let input = dt("2026-04-02T14:30:00Z");
        let result = align_down(input, Granularity::FifteenMin);
        assert_eq!(result, dt("2026-04-02T14:30:00Z"));
    }

    #[test]
    fn align_down_thirty_min() {
        let input = dt("2026-04-02T14:45:00Z");
        let result = align_down(input, Granularity::ThirtyMin);
        assert_eq!(result, dt("2026-04-02T14:30:00Z"));
    }

    #[test]
    fn align_down_one_hour() {
        let input = dt("2026-04-02T14:59:59Z");
        let result = align_down(input, Granularity::OneHour);
        assert_eq!(result, dt("2026-04-02T14:00:00Z"));
    }

    #[test]
    fn align_down_four_hours() {
        let input = dt("2026-04-02T11:30:00Z");
        let result = align_down(input, Granularity::FourHours);
        assert_eq!(result, dt("2026-04-02T08:00:00Z"));
    }

    #[test]
    fn align_down_daily() {
        let input = dt("2026-04-02T23:59:59Z");
        let result = align_down(input, Granularity::Daily);
        assert_eq!(result, dt("2026-04-02T00:00:00Z"));
    }

    #[test]
    fn align_down_weekly() {
        // 2026-04-02 is a Thursday, Monday was 2026-03-30
        let input = dt("2026-04-02T12:00:00Z");
        let result = align_down(input, Granularity::Weekly);
        assert_eq!(result, dt("2026-03-30T00:00:00Z"));
    }

    #[test]
    fn align_down_monthly() {
        let input = dt("2026-04-15T12:00:00Z");
        let result = align_down(input, Granularity::Monthly);
        assert_eq!(result, dt("2026-04-01T00:00:00Z"));
    }

    #[test]
    fn step_period_one_hour() {
        let input = dt("2026-04-02T14:00:00Z");
        let result = step_period(input, Granularity::OneHour);
        assert_eq!(result, Some(dt("2026-04-02T15:00:00Z")));
    }

    #[test]
    fn step_period_monthly_wraps_year() {
        let input = dt("2026-12-01T00:00:00Z");
        let result = step_period(input, Granularity::Monthly);
        assert_eq!(result, Some(dt("2027-01-01T00:00:00Z")));
    }

    #[test]
    fn generate_periods_hourly() {
        let from = dt("2026-04-02T10:00:00Z");
        let to = dt("2026-04-02T12:00:00Z");
        let periods = generate_periods(from, to, Granularity::OneHour);
        assert_eq!(periods.len(), 3);
        assert_eq!(periods[0], "2026-04-02T10:00:00Z");
        assert_eq!(periods[1], "2026-04-02T11:00:00Z");
        assert_eq!(periods[2], "2026-04-02T12:00:00Z");
    }

    #[test]
    fn generate_periods_empty_when_from_after_to() {
        let from = dt("2026-04-03T00:00:00Z");
        let to = dt("2026-04-02T00:00:00Z");
        let periods = generate_periods(from, to, Granularity::OneHour);
        assert!(periods.is_empty());
    }
}
