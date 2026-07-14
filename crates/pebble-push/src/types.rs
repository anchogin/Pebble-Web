#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMagicPushConfig {
    pub base_url: String,
    pub token_encrypted: Option<String>,
    pub public_url: String,
    pub is_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMagicPushConfig {
    pub base_url: String,
    pub token: String,
    pub public_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushEmailAddress {
    pub name: Option<String>,
    pub address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushMessage {
    pub id: String,
    pub subject: String,
    pub snippet: String,
    pub from_address: String,
    pub from_name: String,
    pub to_list: Vec<PushEmailAddress>,
    pub body_text: String,
    pub date: i64,
}
