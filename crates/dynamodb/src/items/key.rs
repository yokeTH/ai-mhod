use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyItem {
    pub pk: String,
    pub sk: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    pub user_id: String,
    pub key: String,
    pub name: Option<String>,
    pub created_at: String,
    pub revoked: Option<bool>,
    pub gsi1_pk: String,
    pub gsi1_sk: String,
    pub gsi2_pk: String,
    pub gsi2_sk: String,
}

impl From<model::user::ApiKey> for KeyItem {
    fn from(api_key: model::user::ApiKey) -> Self {
        let pk = format!("KEY#{}", api_key.id);
        let gsi1_pk = format!("USER#{}", api_key.user_id);
        let gsi1_sk = format!("KEY#{}", api_key.id);
        let gsi2_pk = format!("KEYVAL#{}", api_key.key);
        let gsi2_sk = format!("KEY#{}", api_key.id);
        Self {
            sk: pk.clone(),
            pk,
            item_type: "KEY".to_string(),
            id: api_key.id,
            user_id: api_key.user_id,
            key: api_key.key,
            name: api_key.name,
            created_at: api_key.created_at,
            revoked: Some(api_key.revoked),
            gsi1_pk,
            gsi1_sk,
            gsi2_pk,
            gsi2_sk,
        }
    }
}

impl From<KeyItem> for model::user::ApiKey {
    fn from(item: KeyItem) -> Self {
        Self {
            id: item.id,
            user_id: item.user_id,
            key: item.key,
            name: item.name,
            created_at: item.created_at,
            revoked: item.revoked.unwrap_or(true),
        }
    }
}
