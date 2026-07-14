use crate::credentials::{
    decrypt_credentials, encrypt_credentials, AccountCredentials, ImapCredentials, SmtpCredentials,
};
use crate::error::ApiError;
use crate::routes::oauth::{fetch_google_userinfo, google_oauth_manager};
use crate::state::AppStateRef;
use axum::extract::{Path, State};
use axum::Json;
use pebble_core::{new_id, now_timestamp, Account, ProviderType};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize)]
pub struct AccountResponse {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub color: Option<String>,
    pub provider: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<Account> for AccountResponse {
    fn from(a: Account) -> Self {
        let provider = match a.provider {
            ProviderType::Imap => "imap",
            ProviderType::Gmail => "gmail",
            ProviderType::Outlook => "outlook",
        };
        Self {
            id: a.id,
            email: a.email,
            display_name: a.display_name,
            color: a.color,
            provider: provider.to_string(),
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}

/// Full account configuration surfaced to the edit form.
/// Mirrors the frontend `AccountConfig` interface (camelCase JSON).
/// Never includes secrets — IMAP/SMTP fields come from decrypted credentials
/// with the password omitted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountConfigResponse {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub color: Option<String>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imap_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imap_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imap_security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smtp_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smtp_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smtp_security: Option<String>,
}

pub async fn get_account(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<AccountConfigResponse>, ApiError> {
    let store = state.store.clone();
    let account_id_for_query = account_id.clone();
    let account = store
        .with_read_async(move |conn| {
            let result = conn
                .query_row(
                    "SELECT id, email, display_name, color, provider, created_at, updated_at
                     FROM accounts WHERE id = ?1",
                    rusqlite::params![account_id_for_query],
                    |row| {
                        let provider_str: String = row.get(4)?;
                        let provider = match provider_str.as_str() {
                            "gmail" => ProviderType::Gmail,
                            "outlook" => ProviderType::Outlook,
                            _ => ProviderType::Imap,
                        };
                        Ok(Account {
                            id: row.get(0)?,
                            email: row.get(1)?,
                            display_name: row.get(2)?,
                            color: row.get(3)?,
                            provider,
                            created_at: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .optional()?;
            Ok(result)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read account: {e}")))?
        .ok_or_else(|| ApiError::NotFound(format!("Account {account_id} not found")))?;

    let provider = match account.provider {
        ProviderType::Imap => "imap",
        ProviderType::Gmail => "gmail",
        ProviderType::Outlook => "outlook",
    };

    let (username, imap_host, imap_port, imap_security, smtp_host, smtp_port, smtp_security) =
        if matches!(account.provider, ProviderType::Imap) {
            match state.store.get_account_sync_state(&account.id) {
                Ok(Some(ss_json)) => {
                    let ss_val: serde_json::Value = serde_json::from_str(&ss_json)
                        .map_err(|e| ApiError::Internal(format!("Invalid sync state: {e}")))?;
                    let encrypted_hex = ss_val
                        .get("credentials")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            ApiError::BadRequest("No credentials in account".to_string())
                        })?
                        .to_string();
                    match decrypt_credentials(&state.crypto, &encrypted_hex) {
                        Ok(AccountCredentials::Imap { imap, smtp }) => (
                            Some(imap.username),
                            Some(imap.host),
                            Some(imap.port),
                            Some(imap.security),
                            Some(smtp.host),
                            Some(smtp.port),
                            Some(smtp.security),
                        ),
                        Ok(AccountCredentials::Gmail(_)) => {
                            (None, None, None, None, None, None, None)
                        }
                        Err(_) => (None, None, None, None, None, None, None),
                    }
                }
                _ => (None, None, None, None, None, None, None),
            }
        } else {
            (None, None, None, None, None, None, None)
        };

    Ok(Json(AccountConfigResponse {
        id: account.id,
        email: account.email,
        display_name: account.display_name,
        color: account.color,
        provider: provider.to_string(),
        username,
        imap_host,
        imap_port,
        imap_security,
        smtp_host,
        smtp_port,
        smtp_security,
    }))
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub email: String,
    pub display_name: String,
    pub color: Option<String>,
    // Flat fields matching frontend AddAccountRequest
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub imap_security: Option<String>,
    pub smtp_security: Option<String>,
}

impl CreateAccountRequest {
    fn into_credentials(self) -> (String, String, Option<String>, AccountCredentials) {
        let creds = AccountCredentials::Imap {
            imap: ImapCredentials {
                host: self.imap_host,
                port: self.imap_port,
                username: self.username.clone(),
                password: self.password.clone(),
                security: self.imap_security.unwrap_or_else(|| "tls".to_string()),
            },
            smtp: SmtpCredentials {
                host: self.smtp_host,
                port: self.smtp_port,
                username: self.username,
                password: self.password,
                security: self.smtp_security.unwrap_or_else(|| "tls".to_string()),
            },
        };
        (self.email, self.display_name, self.color, creds)
    }
}

pub async fn list_accounts(
    State(state): State<AppStateRef>,
) -> Result<Json<Vec<AccountResponse>>, ApiError> {
    let store = state.store.clone();
    let accounts = store
        .with_read_async(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, email, display_name, color, provider, created_at, updated_at
                 FROM accounts ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                let provider_str: String = row.get(4)?;
                let provider = match provider_str.as_str() {
                    "gmail" => ProviderType::Gmail,
                    "outlook" => ProviderType::Outlook,
                    _ => ProviderType::Imap,
                };
                Ok(Account {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    display_name: row.get(2)?,
                    color: row.get(3)?,
                    provider,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            let mut accounts = Vec::new();
            for row in rows {
                accounts.push(row?);
            }
            Ok(accounts)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list accounts: {e}")))?;

    let response: Vec<AccountResponse> = accounts.into_iter().map(AccountResponse::from).collect();
    Ok(Json(response))
}

pub async fn create_account(
    State(state): State<AppStateRef>,
    Json(body): Json<CreateAccountRequest>,
) -> Result<Json<AccountResponse>, ApiError> {
    let (email, display_name, color, credentials) = body.into_credentials();

    let encrypted = encrypt_credentials(&state.crypto, &credentials)
        .map_err(|e| ApiError::Internal(format!("Encryption failed: {e}")))?;

    let now = now_timestamp();
    let account = Account {
        id: new_id(),
        email,
        display_name,
        color,
        provider: ProviderType::Imap,
        created_at: now,
        updated_at: now,
    };

    let account_clone = account.clone();
    let store = state.store.clone();
    store
        .with_write_async(move |conn| {
            conn.execute(
                "INSERT INTO accounts (id, email, display_name, color, provider, created_at, updated_at, sync_state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    account_clone.id,
                    account_clone.email,
                    account_clone.display_name,
                    account_clone.color.as_deref(),
                    "imap",
                    account_clone.created_at,
                    account_clone.updated_at,
                    serde_json::json!({ "credentials": encrypted }).to_string(),
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create account: {e}")))?;

    // Auto-start sync worker for the new account
    let account_id = account.id.clone();
    let sync_manager = state.sync_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = sync_manager.start_account_sync(&account_id).await {
            tracing::warn!("Failed to auto-start sync for new account {account_id}: {e}");
        }
    });

    Ok(Json(AccountResponse::from(account)))
}

pub async fn delete_account(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Stop sync worker for this account
    state.sync_manager.stop_account_sync(&account_id).await;

    let store = state.store.clone();
    store
        .with_write_async(move |conn| {
            let tx = conn.unchecked_transaction()?;
            // Remove messages associated with this account
            tx.execute(
                "DELETE FROM message_folders WHERE message_id IN (SELECT id FROM messages WHERE account_id = ?1)",
                rusqlite::params![account_id],
            )?;
            tx.execute(
                "DELETE FROM messages WHERE account_id = ?1",
                rusqlite::params![account_id],
            )?;
            // Remove folders for this account
            tx.execute(
                "DELETE FROM folders WHERE account_id = ?1",
                rusqlite::params![account_id],
            )?;
            // Remove the account itself
            tx.execute(
                "DELETE FROM accounts WHERE id = ?1",
                rusqlite::params![account_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete account: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountRequest {
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub color: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub imap_security: Option<String>,
    pub smtp_security: Option<String>,
}

pub async fn update_account(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
    Json(body): Json<UpdateAccountRequest>,
) -> Result<Json<AccountResponse>, ApiError> {
    let store = state.store.clone();
    let now = now_timestamp();
    let aid = account_id.clone();

    // Update basic account fields
    let mut sets = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql + Send>> = Vec::new();

    if let Some(ref email) = body.email {
        sets.push(format!("email = ?{}", params.len() + 1));
        params.push(Box::new(email.clone()));
    }
    if let Some(ref display_name) = body.display_name {
        sets.push(format!("display_name = ?{}", params.len() + 1));
        params.push(Box::new(display_name.clone()));
    }
    if let Some(ref color) = body.color {
        sets.push(format!("color = ?{}", params.len() + 1));
        params.push(Box::new(color.clone()));
    }

    sets.push(format!("updated_at = ?{}", params.len() + 1));
    params.push(Box::new(now));

    let aid_param_idx = params.len() + 1;
    params.push(Box::new(aid.clone()));

    let sql = format!(
        "UPDATE accounts SET {} WHERE id = ?{}",
        sets.join(", "),
        aid_param_idx
    );

    store
        .with_write_async(move |conn| {
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
                .iter()
                .map(|p| p.as_ref() as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(&sql, param_refs.as_slice())?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update account: {e}")))?;

    // If credential fields changed, update sync_state
    let has_cred_changes = body.password.is_some()
        || body.username.is_some()
        || body.imap_host.is_some()
        || body.imap_port.is_some()
        || body.smtp_host.is_some()
        || body.smtp_port.is_some()
        || body.imap_security.is_some()
        || body.smtp_security.is_some();

    if has_cred_changes {
        let store2 = state.store.clone();
        let aid2 = account_id.clone();
        let crypto = state.crypto.clone();

        // Read current credentials
        let sync_state_json = store2
            .with_read_async(move |conn| {
                let result: Option<Option<String>> = conn
                    .query_row(
                        "SELECT sync_state FROM accounts WHERE id = ?1",
                        rusqlite::params![aid2],
                        |row| row.get(0),
                    )
                    .optional()?;
                Ok(result.flatten())
            })
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to read account: {e}")))?;

        if let Some(ss_json) = sync_state_json {
            let sync_state: serde_json::Value = serde_json::from_str(&ss_json)
                .map_err(|e| ApiError::Internal(format!("Invalid sync state: {e}")))?;

            if let Some(encrypted_hex) = sync_state["credentials"].as_str() {
                let mut creds = crate::credentials::decrypt_credentials(&crypto, encrypted_hex)
                    .map_err(|e| ApiError::Internal(format!("Decrypt failed: {e}")))?;

                match &mut creds {
                    AccountCredentials::Imap { imap, smtp } => {
                        if let Some(ref p) = body.password {
                            imap.password = p.clone();
                            smtp.password = p.clone();
                        }
                        if let Some(ref username) = body.username {
                            imap.username = username.clone();
                            smtp.username = username.clone();
                        }
                        if let Some(ref h) = body.imap_host {
                            imap.host = h.clone();
                        }
                        if let Some(p) = body.imap_port {
                            imap.port = p;
                        }
                        if let Some(ref s) = body.imap_security {
                            imap.security = s.clone();
                        }
                        if let Some(ref h) = body.smtp_host {
                            smtp.host = h.clone();
                        }
                        if let Some(p) = body.smtp_port {
                            smtp.port = p;
                        }
                        if let Some(ref s) = body.smtp_security {
                            smtp.security = s.clone();
                        }
                    }
                    AccountCredentials::Gmail(_) => {
                        // Gmail credentials are not updated here.
                    }
                }

                let encrypted = encrypt_credentials(&crypto, &creds)
                    .map_err(|e| ApiError::Internal(format!("Encrypt failed: {e}")))?;

                let store3 = state.store.clone();
                let aid3 = account_id.clone();
                store3
                    .with_write_async(move |conn| {
                        conn.execute(
                            "UPDATE accounts SET sync_state = ?1 WHERE id = ?2",
                            rusqlite::params![
                                {
                                    let mut updated = sync_state;
                                    updated["credentials"] = serde_json::Value::String(encrypted);
                                    updated.to_string()
                                },
                                aid3,
                            ],
                        )?;
                        Ok(())
                    })
                    .await
                    .map_err(|e| {
                        ApiError::Internal(format!("Failed to update credentials: {e}"))
                    })?;

                // Restart sync worker with new credentials
                let sync_manager = state.sync_manager.clone();
                let aid4 = account_id.clone();
                tokio::spawn(async move {
                    sync_manager.stop_account_sync(&aid4).await;
                    if let Err(e) = sync_manager.start_account_sync(&aid4).await {
                        tracing::warn!("Failed to restart sync after credential update: {e}");
                    }
                });
            }
        }
    }

    // Return updated account
    let store_final = state.store.clone();
    let aid_final = account_id.clone();
    let account = store_final
        .with_read_async(move |conn| {
            let row = conn.query_row(
                "SELECT id, email, display_name, color, provider, created_at, updated_at FROM accounts WHERE id = ?1",
                rusqlite::params![aid_final],
                |row| {
                    let provider_str: String = row.get(4)?;
                    let provider = match provider_str.as_str() {
                        "gmail" => ProviderType::Gmail,
                        "outlook" => ProviderType::Outlook,
                        _ => ProviderType::Imap,
                    };
                    Ok(Account {
                        id: row.get(0)?,
                        email: row.get(1)?,
                        display_name: row.get(2)?,
                        color: row.get(3)?,
                        provider,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )?;
            Ok(row)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read updated account: {e}")))?;

    Ok(Json(AccountResponse::from(account)))
}

pub async fn test_account_connection(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ss_json = state
        .store
        .get_account_sync_state(&account_id)
        .map_err(|e| ApiError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Account sync state not found".to_string()))?;

    let encrypted_hex = serde_json::from_str::<serde_json::Value>(&ss_json)
        .map_err(|_| ApiError::Internal("Invalid sync state JSON".to_string()))?
        .get("credentials")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("No credentials in account".to_string()))?
        .to_string();

    let mut creds = decrypt_credentials(&state.crypto, &encrypted_hex)
        .map_err(|e| ApiError::Internal(format!("Failed to decrypt credentials: {e}")))?;

    match &mut creds {
        AccountCredentials::Imap { imap, .. } => {
            let security = match imap.security.as_str() {
                "starttls" => pebble_mail::ConnectionSecurity::StartTls,
                "plain" => pebble_mail::ConnectionSecurity::Plain,
                _ => pebble_mail::ConnectionSecurity::Tls,
            };

            let config = pebble_mail::ImapConfig {
                host: imap.host.clone(),
                port: imap.port,
                username: imap.username.clone(),
                password: imap.password.clone(),
                security,
                proxy: None, // TODO: support proxy
            };

            match pebble_mail::ImapProvider::test_connection_with_login(&config).await {
                Ok(report) => Ok(Json(json!({ "success": true, "message": report }))),
                Err(e) => Err(ApiError::BadRequest(format!(
                    "IMAP connection failed: {}",
                    e
                ))),
            }
        }
        AccountCredentials::Gmail(gmail_creds) => {
            // Test current token
            if fetch_google_userinfo(&gmail_creds.access_token)
                .await
                .is_ok()
            {
                return Ok(Json(
                    json!({ "success": true, "message": "Gmail token is valid." }),
                ));
            }

            // Token failed, try to refresh
            let refresh_token = match gmail_creds.refresh_token.as_deref() {
                Some(rt) => rt.to_string(),
                None => {
                    return Err(ApiError::BadRequest(
                        "Gmail token is invalid and no refresh token is available.".to_string(),
                    ));
                }
            };

            let manager = google_oauth_manager(&state.config)?;
            let new_pair = manager
                .refresh_token(&refresh_token)
                .await
                .map_err(|e| ApiError::BadRequest(format!("Gmail token refresh failed: {e}")))?;

            // Refresh succeeded, update credentials
            gmail_creds.access_token = new_pair.access_token;
            gmail_creds.expires_at = new_pair.expires_at;

            let encrypted = encrypt_credentials(&state.crypto, &creds)
                .map_err(|e| ApiError::Internal(format!("Encrypt failed: {e}")))?;

            let mut sync_state_val: serde_json::Value = serde_json::from_str(&ss_json)
                .map_err(|_| ApiError::Internal("Invalid sync state JSON".to_string()))?;

            sync_state_val["credentials"] = serde_json::Value::String(encrypted);

            state
                .store
                .update_account_sync_state(&account_id, &sync_state_val.to_string())
                .map_err(|e| ApiError::Internal(format!("Failed to save new token: {e}")))?;

            Ok(Json(
                json!({ "success": true, "message": "Gmail token was expired but successfully refreshed." }),
            ))
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestImapRequest {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

pub async fn test_imap_connection(
    Json(body): Json<TestImapRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let security = match body.imap_security.as_deref() {
        Some("starttls") => pebble_mail::ConnectionSecurity::StartTls,
        Some("plain") => pebble_mail::ConnectionSecurity::Plain,
        _ => pebble_mail::ConnectionSecurity::Tls,
    };

    let config = pebble_mail::ImapConfig {
        host: body.imap_host,
        port: body.imap_port,
        username: body.username.unwrap_or_default(),
        password: body.password.unwrap_or_default(),
        security,
        proxy: None,
    };

    match pebble_mail::ImapProvider::test_connection_with_login(&config).await {
        Ok(report) => Ok(Json(serde_json::json!({ "ok": true, "report": report }))),
        Err(e) => Err(ApiError::BadRequest(format!("Connection failed: {e}"))),
    }
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{
        encrypt_credentials, AccountCredentials, ImapCredentials, SmtpCredentials,
    };
    use axum::extract::{Path, State};
    use pebble_core::{now_timestamp, Account, ProviderType};
    use serde_json::Value;

    #[tokio::test]
    async fn update_account_credentials_preserves_sync_state_metadata() {
        let state = crate::routes::magicpush::tests::test_state_ref();
        let account = Account {
            id: "account-1".to_string(),
            email: "alice@example.com".to_string(),
            display_name: "Alice".to_string(),
            color: None,
            provider: ProviderType::Imap,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };
        state.store.insert_account(&account).unwrap();
        let encrypted = encrypt_credentials(
            &state.crypto,
            &AccountCredentials::Imap {
                imap: ImapCredentials {
                    host: "imap.old.example.com".to_string(),
                    port: 993,
                    username: "alice@example.com".to_string(),
                    password: "old-password".to_string(),
                    security: "tls".to_string(),
                },
                smtp: SmtpCredentials {
                    host: "smtp.old.example.com".to_string(),
                    port: 465,
                    username: "alice@example.com".to_string(),
                    password: "old-password".to_string(),
                    security: "tls".to_string(),
                },
            },
        )
        .unwrap();
        state
            .store
            .update_account_sync_state(
                &account.id,
                &serde_json::json!({
                    "credentials": encrypted,
                    "sync_strategy": "since_date",
                    "sync_since_date": "2024-01-01",
                    "provider": "imap"
                })
                .to_string(),
            )
            .unwrap();

        let _ = update_account(
            State(state.clone()),
            Path(account.id.clone()),
            Json(UpdateAccountRequest {
                email: Some("alice@example.com".to_string()),
                display_name: Some("Alice".to_string()),
                color: None,
                username: None,
                password: None,
                imap_host: Some("imap.new.example.com".to_string()),
                imap_port: None,
                smtp_host: None,
                smtp_port: None,
                imap_security: None,
                smtp_security: None,
            }),
        )
        .await
        .unwrap();

        let stored = state
            .store
            .get_account_sync_state(&account.id)
            .unwrap()
            .unwrap();
        let state_json: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(state_json["sync_strategy"], "since_date");
        assert_eq!(state_json["sync_since_date"], "2024-01-01");
        assert_eq!(state_json["provider"], "imap");
        assert!(state_json["credentials"].as_str().is_some());
    }

    #[tokio::test]
    async fn get_account_config_returns_username_without_password() {
        let state = crate::routes::magicpush::tests::test_state_ref();
        let account = Account {
            id: "account-1".to_string(),
            email: "qq-user@qq.com".to_string(),
            display_name: "QQ User".to_string(),
            color: None,
            provider: ProviderType::Imap,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };
        state.store.insert_account(&account).unwrap();
        let encrypted = encrypt_credentials(
            &state.crypto,
            &AccountCredentials::Imap {
                imap: ImapCredentials {
                    host: "imap.qq.com".to_string(),
                    port: 993,
                    username: "qq-user@qq.com".to_string(),
                    password: "authorization-code".to_string(),
                    security: "tls".to_string(),
                },
                smtp: SmtpCredentials {
                    host: "smtp.qq.com".to_string(),
                    port: 465,
                    username: "qq-user@qq.com".to_string(),
                    password: "authorization-code".to_string(),
                    security: "tls".to_string(),
                },
            },
        )
        .unwrap();
        state
            .store
            .update_account_sync_state(
                &account.id,
                &serde_json::json!({ "credentials": encrypted }).to_string(),
            )
            .unwrap();

        let Json(response) = get_account(State(state), Path(account.id.clone()))
            .await
            .unwrap();
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["username"], "qq-user@qq.com");
        assert!(json.get("password").is_none());
    }
}
