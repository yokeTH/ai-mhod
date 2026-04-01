use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserItem {
    pub pk: String,
    pub sk: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub keycloak_sub: Option<String>,
    pub gsi1_pk: String,
    pub gsi1_sk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gsi3_pk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gsi3_sk: Option<String>,
}

impl From<model::user::User> for UserItem {
    fn from(user: model::user::User) -> Self {
        let pk = format!("USER#{}", user.id);
        let gsi1_pk = format!("USERNAME#{}", user.name);
        let gsi1_sk = format!("USER#{}", user.id);
        let gsi3_pk = user.keycloak_sub.as_ref().map(|sub| format!("KC#{sub}"));
        let gsi3_sk = gsi3_pk.clone();
        Self {
            sk: pk.clone(),
            pk,
            item_type: "USER".to_string(),
            id: user.id,
            name: user.name,
            created_at: user.created_at,
            keycloak_sub: user.keycloak_sub,
            gsi1_pk,
            gsi1_sk,
            gsi3_pk,
            gsi3_sk,
        }
    }
}

impl From<UserItem> for model::user::User {
    fn from(item: UserItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            created_at: item.created_at,
            keycloak_sub: item.keycloak_sub,
        }
    }
}
