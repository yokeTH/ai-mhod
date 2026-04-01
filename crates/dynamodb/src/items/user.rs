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
}

impl From<model::user::User> for UserItem {
    fn from(user: model::user::User) -> Self {
        let pk = format!("USER#{}", user.id);
        let gsi1_pk = format!("USERNAME#{}", user.name);
        let gsi1_sk = format!("USER#{}", user.id);
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
