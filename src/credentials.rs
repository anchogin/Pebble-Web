use pebble_crypto::CryptoService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapCredentials {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub security: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpCredentials {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub security: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AccountCredentials {
    #[serde(rename = "imap")]
    Imap {
        imap: ImapCredentials,
        smtp: SmtpCredentials,
    },
    #[serde(rename = "gmail")]
    Gmail(GmailCredentials),
}

pub fn encrypt_credentials(
    crypto: &CryptoService,
    creds: &AccountCredentials,
) -> Result<String, String> {
    let json = serde_json::to_vec(creds).map_err(|e| e.to_string())?;
    let encrypted = crypto.encrypt(&json).map_err(|e| format!("{e}"))?;
    Ok(hex::encode(encrypted))
}

pub fn decrypt_credentials(
    crypto: &CryptoService,
    encrypted_hex: &str,
) -> Result<AccountCredentials, String> {
    let encrypted = hex::decode(encrypted_hex).map_err(|e| format!("Invalid hex: {e}"))?;
    let decrypted = crypto.decrypt(&encrypted).map_err(|e| format!("{e}"))?;
    serde_json::from_slice(&decrypted).map_err(|e| format!("Invalid JSON: {e}"))
}
