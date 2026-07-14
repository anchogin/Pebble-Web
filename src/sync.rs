use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pebble_crypto::CryptoService;
use pebble_mail::{
    gmail_sync::GmailSyncWorker, provider::gmail::GmailProvider, ConnectionSecurity, ImapConfig,
    ImapMailProvider, StoredMessage, SyncConfig, SyncTrigger, SyncWorker,
};
use pebble_oauth::{OAuthConfig, OAuthManager, OAuthNetworkConfig};
use pebble_store::Store;
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::credentials::{
    decrypt_credentials, AccountCredentials, GmailCredentials, ImapCredentials,
};
use crate::magicpush::MagicPushNotifier;

/// Handle for a running sync worker task.
pub struct SyncHandle {
    pub stop_tx: watch::Sender<bool>,
    pub trigger_tx: mpsc::UnboundedSender<SyncTrigger>,
    pub task: JoinHandle<()>,
    pub cancel_tx: watch::Sender<bool>,
}

/// Manages background IMAP sync workers for all configured accounts.
pub struct SyncManager {
    handles: Mutex<HashMap<String, SyncHandle>>,
    store: Arc<Store>,
    crypto: Arc<CryptoService>,
    attachments_dir: PathBuf,
    sync_interval_secs: u64,
    ws_tx: broadcast::Sender<String>,
    magicpush: MagicPushNotifier,
}

impl SyncManager {
    pub fn new(
        store: Arc<Store>,
        crypto: Arc<CryptoService>,
        attachments_dir: PathBuf,
        sync_interval_secs: u64,
        ws_tx: broadcast::Sender<String>,
        magicpush: MagicPushNotifier,
    ) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            store,
            crypto,
            attachments_dir,
            sync_interval_secs,
            ws_tx,
            magicpush,
        }
    }

    pub fn sync_interval_secs(&self) -> u64 {
        self.sync_interval_secs
    }

    /// Start sync workers for all configured accounts.
    pub async fn start_all(&self) {
        let accounts = match self.store.list_accounts() {
            Ok(accounts) => accounts,
            Err(e) => {
                error!("Failed to list accounts for sync startup: {e}");
                return;
            }
        };

        for account in accounts {
            if let Err(e) = self.start_account_sync(&account.id).await {
                error!("Failed to start sync for account {}: {e}", account.id);
            }
        }
    }

    /// Start sync for a single account by ID.
    pub async fn start_account_sync(&self, account_id: &str) -> Result<(), String> {
        self.start_account_sync_inner(account_id, true)
            .await
            .map(|_| ())
    }

    async fn start_account_sync_if_missing(&self, account_id: &str) -> Result<bool, String> {
        self.start_account_sync_inner(account_id, false).await
    }

    async fn start_account_sync_inner(
        &self,
        account_id: &str,
        replace_existing: bool,
    ) -> Result<bool, String> {
        let mut handles = self.handles.lock().await;

        if handles.contains_key(account_id) && !replace_existing {
            return Ok(false);
        }

        if let Some(handle) = handles.remove(account_id) {
            let _ = handle.stop_tx.send(true);
            handle.task.abort();
        }

        // Get credentials from sync_state.
        let sync_state_json = self
            .store
            .get_account_sync_state(account_id)
            .map_err(|e| format!("Failed to get sync state: {e}"))?
            .ok_or_else(|| "No sync state found for account".to_string())?;

        let sync_state: serde_json::Value = serde_json::from_str(&sync_state_json)
            .map_err(|e| format!("Invalid sync state JSON: {e}"))?;

        let encrypted_hex = sync_state["credentials"]
            .as_str()
            .ok_or_else(|| "No credentials in sync state".to_string())?;

        let creds = decrypt_credentials(&self.crypto, encrypted_hex)
            .map_err(|e| format!("Failed to decrypt credentials: {e}"))?;

        match creds {
            AccountCredentials::Imap { ref imap, .. } => {
                self.start_imap_worker(account_id, imap.clone(), sync_state, handles)
                    .await?;
            }
            AccountCredentials::Gmail(gmail_creds) => {
                self.start_gmail_worker(account_id, gmail_creds, sync_state, handles)
                    .await?;
            }
        }
        Ok(true)
    }

    async fn start_imap_worker(
        &self,
        account_id: &str,
        imap_creds: ImapCredentials,
        _sync_state: serde_json::Value,
        mut handles: tokio::sync::MutexGuard<'_, HashMap<String, SyncHandle>>,
    ) -> Result<(), String> {
        let imap_config = build_imap_config(&imap_creds);
        let provider = Arc::new(ImapMailProvider::new(imap_config));

        let (stop_tx, stop_rx) = watch::channel(false);
        let (trigger_tx, trigger_rx) = mpsc::unbounded_channel();

        let worker = SyncWorker::new(
            account_id.to_string(),
            provider,
            self.store.clone(),
            stop_rx,
            self.attachments_dir.clone(),
        );

        let cancel_tx = worker.cancel_sender();

        // Create progress channel and forward to WebSocket
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<pebble_mail::SyncProgress>();
        let worker = worker.with_progress_tx(progress_tx);

        // Create log channel and forward to WebSocket
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<pebble_mail::SyncLogEntry>();
        let worker = worker.with_log_tx(log_tx);

        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<StoredMessage>();
        let worker = worker.with_message_tx(message_tx);

        let sync_config = SyncConfig {
            poll_interval_secs: self.sync_interval_secs,
            ..SyncConfig::default()
        };

        let account_id_owned = account_id.to_string();
        let ws_tx = self.ws_tx.clone();
        let magicpush = self.magicpush.clone();
        let magicpush_store = self.store.clone();
        let task = tokio::spawn(async move {
            info!("Sync worker started for account {}", account_id_owned);
            let _ = ws_tx.send(
                serde_json::json!({
                    "type": "sync_started",
                    "account_id": account_id_owned,
                })
                .to_string(),
            );

            // Spawn a task to forward progress messages to WebSocket
            let progress_ws_tx = ws_tx.clone();
            let progress_account_id = account_id_owned.clone();
            tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let msg = serde_json::json!({
                        "type": "sync_progress",
                        "account_id": progress_account_id,
                        "status": progress.status,
                        "phase": progress.phase,
                        "message": progress.message,
                        "progress": progress.progress.map(|p| {
                            serde_json::json!({
                                "current": p.current,
                                "total": p.total,
                                "percentage": p.percentage,
                            })
                        }),
                    });
                    let _ = progress_ws_tx.send(msg.to_string());
                }
            });

            // Spawn a task to forward log messages to WebSocket
            let log_ws_tx = ws_tx.clone();
            let log_account_id = account_id_owned.clone();
            tokio::spawn(async move {
                while let Some(log_entry) = log_rx.recv().await {
                    let msg = serde_json::json!({
                        "type": "sync_log",
                        "account_id": log_account_id,
                        "log": {
                            "timestamp": log_entry.timestamp,
                            "level": log_entry.level,
                            "server": log_entry.server,
                            "action": log_entry.action,
                            "request": log_entry.request,
                            "response": log_entry.response,
                            "error": log_entry.error,
                            "message_count": log_entry.message_count,
                        },
                    });
                    let _ = log_ws_tx.send(msg.to_string());
                }
            });

            let message_ws_tx = ws_tx.clone();
            tokio::spawn(async move {
                while let Some(stored) = message_rx.recv().await {
                    forward_synced_message(
                        &message_ws_tx,
                        &magicpush,
                        magicpush_store.as_ref(),
                        &stored,
                    )
                    .await;
                }
            });

            worker.run(sync_config, Some(trigger_rx)).await;
            let _ = ws_tx.send(
                serde_json::json!({
                    "type": "sync_complete",
                    "account_id": account_id_owned,
                })
                .to_string(),
            );
            info!("Sync worker stopped for account {}", account_id_owned);
        });

        handles.insert(
            account_id.to_string(),
            SyncHandle {
                stop_tx,
                trigger_tx: trigger_tx.clone(),
                task,
                cancel_tx,
            },
        );
        info!(
            "[SyncManager] IMAP worker handle inserted for account: {}",
            account_id
        );

        info!("Started sync for account {}", account_id);
        Ok(())
    }

    async fn start_gmail_worker(
        &self,
        account_id: &str,
        gmail_creds: GmailCredentials,
        sync_state: serde_json::Value,
        mut handles: tokio::sync::MutexGuard<'_, HashMap<String, SyncHandle>>,
    ) -> Result<(), String> {
        info!(
            "[SyncManager] Starting Gmail worker for account: {}",
            account_id
        );

        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<pebble_mail::SyncProgress>();
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<pebble_mail::SyncLogEntry>();
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<StoredMessage>();

        let provider = Arc::new(
            GmailProvider::new(gmail_creds.access_token.clone()).with_log_tx(log_tx.clone()),
        );

        let (stop_tx, stop_rx) = watch::channel(false);
        let (trigger_tx, trigger_rx) = mpsc::unbounded_channel::<SyncTrigger>();

        let mut worker = GmailSyncWorker::new(
            account_id.to_string(),
            provider.clone(),
            self.store.clone(),
            stop_rx,
            self.attachments_dir.clone(),
        )
        .with_progress_tx(progress_tx)
        .with_log_tx(log_tx.clone())
        .with_message_tx(message_tx);

        let cancel_tx = worker.cancel_sender();

        let crypto = self.crypto.clone();
        let store = self.store.clone();
        let account_id_str = account_id.to_string();

        if let Some(refresh_token) = gmail_creds.refresh_token.clone() {
            let client_id = sync_state["google_client_id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let client_secret = sync_state["google_client_secret"]
                .as_str()
                .map(String::from);

            let oauth_config = OAuthConfig {
                client_id,
                client_secret,
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
                scopes: vec!["https://mail.google.com/".to_string()],
                redirect_port: 0,
            };
            let oauth_manager = Arc::new(OAuthManager::new_with_network(
                oauth_config,
                OAuthNetworkConfig::default(),
            ));
            let expires_at = gmail_creds.expires_at;

            let refresher: pebble_mail::gmail_sync::TokenRefresher = {
                let provider_ref = provider.clone();
                let account_id_ref = account_id_str.clone();
                let oauth_manager = oauth_manager.clone();
                let refresh_token = refresh_token.clone();
                let crypto = crypto.clone();
                let store = store.clone();
                Box::new(move || {
                    let oauth_manager = oauth_manager.clone();
                    let refresh_token = refresh_token.clone();
                    let provider_ref = provider_ref.clone();
                    let account_id_ref = account_id_ref.clone();
                    let crypto = crypto.clone();
                    let store = store.clone();
                    Box::pin(async move {
                        let pair = oauth_manager
                            .refresh_token(&refresh_token)
                            .await
                            .map_err(|e| pebble_core::PebbleError::Auth(e.to_string()))?;
                        provider_ref.set_access_token(pair.access_token.clone());
                        if let Ok(Some(sync_json)) = store.get_account_sync_state(&account_id_ref) {
                            if let Ok(mut state) =
                                serde_json::from_str::<serde_json::Value>(&sync_json)
                            {
                                if let Some(encrypted_hex) = state["credentials"].as_str() {
                                    if let Ok(creds) = crate::credentials::decrypt_credentials(
                                        &crypto,
                                        encrypted_hex,
                                    ) {
                                        if let AccountCredentials::Gmail(mut g) = creds {
                                            g.access_token = pair.access_token.clone();
                                            if let Some(exp) = pair.expires_at {
                                                g.expires_at = Some(exp);
                                            }
                                            if let Ok(new_enc) =
                                                crate::credentials::encrypt_credentials(
                                                    &crypto,
                                                    &AccountCredentials::Gmail(g),
                                                )
                                            {
                                                state["credentials"] = serde_json::json!(new_enc);
                                                let _ = store.update_account_sync_state(
                                                    &account_id_ref,
                                                    &state.to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok((pair.access_token, pair.expires_at))
                    })
                })
            };
            worker = worker.with_token_refresher(refresher, expires_at);
        }

        let sync_config = build_gmail_sync_config(&sync_state, self.sync_interval_secs);

        let account_id_owned = account_id.to_string();
        let ws_tx = self.ws_tx.clone();
        let magicpush = self.magicpush.clone();
        let magicpush_store = self.store.clone();
        let task = tokio::spawn(async move {
            info!("Gmail sync worker started for account {}", account_id_owned);
            let _ = ws_tx.send(
                serde_json::json!({
                    "type": "sync_started",
                    "account_id": account_id_owned,
                })
                .to_string(),
            );

            let progress_ws_tx = ws_tx.clone();
            let progress_account_id = account_id_owned.clone();
            tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let msg = serde_json::json!({
                        "type": "sync_progress",
                        "account_id": progress_account_id,
                        "status": progress.status,
                        "phase": progress.phase,
                        "message": progress.message,
                        "progress": progress.progress.map(|p| serde_json::json!({
                            "current": p.current,
                            "total": p.total,
                            "percentage": p.percentage,
                        })),
                    });
                    let _ = progress_ws_tx.send(msg.to_string());
                }
            });

            let log_ws_tx = ws_tx.clone();
            let log_account_id = account_id_owned.clone();
            tokio::spawn(async move {
                while let Some(log_entry) = log_rx.recv().await {
                    let msg = serde_json::json!({
                        "type": "sync_log",
                        "account_id": log_account_id,
                        "log": {
                            "timestamp": log_entry.timestamp,
                            "level": log_entry.level,
                            "server": log_entry.server,
                            "action": log_entry.action,
                            "request": log_entry.request,
                            "response": log_entry.response,
                            "error": log_entry.error,
                            "message_count": log_entry.message_count,
                        },
                    });
                    let _ = log_ws_tx.send(msg.to_string());
                }
            });

            let message_ws_tx = ws_tx.clone();
            tokio::spawn(async move {
                while let Some(stored) = message_rx.recv().await {
                    forward_synced_message(
                        &message_ws_tx,
                        &magicpush,
                        magicpush_store.as_ref(),
                        &stored,
                    )
                    .await;
                }
            });

            worker.run(sync_config, Some(trigger_rx)).await;
            let _ = ws_tx.send(
                serde_json::json!({
                    "type": "sync_complete",
                    "account_id": account_id_owned,
                })
                .to_string(),
            );
            info!("Gmail sync worker stopped for account {}", account_id_owned);
        });

        handles.insert(
            account_id.to_string(),
            SyncHandle {
                stop_tx,
                trigger_tx: trigger_tx.clone(),
                task,
                cancel_tx,
            },
        );

        info!("Started Gmail sync for account {}", account_id);

        Ok(())
    }

    /// Stop sync for a single account.
    pub async fn stop_account_sync(&self, account_id: &str) {
        let mut handles = self.handles.lock().await;
        if let Some(handle) = handles.remove(account_id) {
            info!("Stopping sync for account {}", account_id);
            let _ = handle.stop_tx.send(true);
            handle.task.abort();
        }
    }

    /// Trigger a manual sync for a specific account.
    pub async fn trigger_sync(&self, account_id: &str) -> Result<(), String> {
        self.trigger_sync_with_reason(account_id, SyncTrigger::Manual)
            .await
    }

    pub async fn trigger_sync_with_reason(
        &self,
        account_id: &str,
        trigger: SyncTrigger,
    ) -> Result<(), String> {
        info!("[SyncManager] Received trigger for account: {}", account_id);
        let needs_start = {
            let mut handles = self.handles.lock().await;
            match handles.get(account_id) {
                Some(handle) => {
                    info!("[SyncManager] Found handle, sending sync trigger...");
                    if let Err(e) = handle.trigger_tx.send(trigger) {
                        error!(
                            "[SyncManager] Failed to send trigger: {}. Channel may be closed.",
                            e
                        );
                        if let Some(stale) = handles.remove(account_id) {
                            let _ = stale.stop_tx.send(true);
                            stale.task.abort();
                        }
                        true
                    } else {
                        info!("[SyncManager] Trigger sent successfully.");
                        false
                    }
                }
                None => {
                    warn!(
                        "[SyncManager] No sync worker handle found for account {}; starting worker before trigger",
                        account_id
                    );
                    true
                }
            }
        };

        if needs_start {
            warn!(
                "[SyncManager] Starting missing or closed sync worker for account {} before retrying trigger",
                account_id
            );
            self.start_account_sync_if_missing(account_id).await?;
            if !should_resend_trigger_after_worker_start(trigger) {
                let _ = self.ws_tx.send(
                    serde_json::json!({
                        "type": "sync_started",
                        "account_id": account_id,
                    })
                    .to_string(),
                );
                return Ok(());
            }
            let handles = self.handles.lock().await;
            let Some(handle) = handles.get(account_id) else {
                return Err(format!("No sync worker running for account {account_id}"));
            };
            handle
                .trigger_tx
                .send(trigger)
                .map_err(|_| "Sync worker channel closed after restart".to_string())?;
            info!("[SyncManager] Trigger sent successfully after worker restart.");
        }

        let _ = self.ws_tx.send(
            serde_json::json!({
                "type": "sync_started",
                "account_id": account_id,
            })
            .to_string(),
        );

        Ok(())
    }

    /// Get the cancel channel sender for a running sync worker.
    pub fn get_sync_worker(&self, _account_id: &str) -> Option<SyncWorkerHandle> {
        // This is a simplified version - we'll store a reference to the worker
        // For now, we'll use the cancel_tx from the handle
        // Actually, we need to redesign this - let me add a method to request cancellation directly
        None
    }

    /// Request cancellation of sync for an account.
    pub async fn request_cancel_sync(&self, account_id: &str) -> bool {
        let handles = self.handles.lock().await;
        if let Some(handle) = handles.get(account_id) {
            let _ = handle.cancel_tx.send(true);
            true
        } else {
            false
        }
    }
}

fn should_resend_trigger_after_worker_start(trigger: SyncTrigger) -> bool {
    trigger != SyncTrigger::Startup
}

async fn forward_magicpush_message(
    notifier: &MagicPushNotifier,
    store: &Store,
    stored: &StoredMessage,
) {
    if !is_received_message(store, stored) {
        return;
    }
    if let Err(e) = notifier.send_stored_message(stored).await {
        warn!(message_id = %stored.message.id, error = %e, "MagicPush notification failed");
    }
}

async fn forward_synced_message(
    ws_tx: &broadcast::Sender<String>,
    notifier: &MagicPushNotifier,
    store: &Store,
    stored: &StoredMessage,
) {
    emit_new_mail_event(ws_tx, stored);
    forward_magicpush_message(notifier, store, stored).await;
}

fn emit_new_mail_event(ws_tx: &broadcast::Sender<String>, stored: &StoredMessage) {
    let _ = ws_tx.send(
        serde_json::json!({
            "type": "new_mail",
            "account_id": stored.message.account_id,
            "message_id": stored.message.id,
            "folder_ids": stored.folder_ids,
            "notify": stored.notify,
        })
        .to_string(),
    );
}

fn is_received_message(store: &Store, stored: &StoredMessage) -> bool {
    stored.notify
        && stored.folder_ids.iter().any(|folder_id| {
            store
                .find_folder_by_id(folder_id)
                .ok()
                .flatten()
                .and_then(|folder| folder.role)
                == Some(pebble_core::FolderRole::Inbox)
        })
}

/// Handle for accessing a running sync worker
pub struct SyncWorkerHandle {
    cancel_tx: watch::Sender<bool>,
}

impl SyncWorkerHandle {
    pub fn request_cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }
}

/// Convert ImapCredentials to ImapConfig for pebble-mail.
fn build_imap_config(creds: &ImapCredentials) -> ImapConfig {
    let security = match creds.security.as_str() {
        "starttls" => ConnectionSecurity::StartTls,
        "plain" => ConnectionSecurity::Plain,
        _ => ConnectionSecurity::Tls,
    };

    ImapConfig {
        host: creds.host.clone(),
        port: creds.port,
        username: creds.username.clone(),
        password: creds.password.clone(),
        security,
        proxy: None,
    }
}

fn build_gmail_sync_config(sync_state: &serde_json::Value, poll_interval_secs: u64) -> SyncConfig {
    let _ = sync_state;
    SyncConfig {
        poll_interval_secs,
        ..SyncConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::encrypt_credentials;
    use crate::magicpush::encrypt_magicpush_token;
    use axum::{body::Bytes, extract::State, http::StatusCode, routing::post, Json, Router};
    use pebble_core::{
        Account, AppSettingsRecord, EmailAddress, Folder, FolderRole, FolderType,
        MagicPushConfigRecord, Message, ProviderType,
    };
    use pebble_crypto::CryptoService;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    const TEST_SYNC_INTERVAL_SECS: u64 = 300;

    #[tokio::test]
    async fn trigger_sync_restarts_closed_worker_channel() {
        let (store, crypto) = test_store();
        let credentials = AccountCredentials::Gmail(GmailCredentials {
            access_token: "expired-access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(0),
            email: "alice@example.com".to_string(),
        });
        let encrypted = encrypt_credentials(&crypto, &credentials).unwrap();
        store
            .update_account_sync_state(
                "account-1",
                &serde_json::json!({
                    "credentials": encrypted,
                    "google_client_id": "google-client-id",
                    "google_client_secret": "google-client-secret",
                })
                .to_string(),
            )
            .unwrap();
        let store = Arc::new(store);
        let (ws_tx, _) = broadcast::channel(16);
        let magicpush = MagicPushNotifier::new(store.clone(), crypto.clone()).unwrap();
        let manager = SyncManager::new(
            store,
            crypto,
            PathBuf::new(),
            TEST_SYNC_INTERVAL_SECS,
            ws_tx,
            magicpush,
        );
        let (stop_tx, _) = watch::channel(false);
        let (trigger_tx, trigger_rx) = mpsc::unbounded_channel();
        drop(trigger_rx);
        let (cancel_tx, _) = watch::channel(false);

        manager.handles.lock().await.insert(
            "account-1".to_string(),
            SyncHandle {
                stop_tx,
                trigger_tx,
                task: tokio::spawn(async {}),
                cancel_tx,
            },
        );

        manager.trigger_sync("account-1").await.unwrap();
        manager.stop_account_sync("account-1").await;
    }

    #[tokio::test]
    async fn trigger_sync_starts_missing_worker_for_existing_account() {
        let (store, crypto) = test_store();
        let credentials = AccountCredentials::Gmail(GmailCredentials {
            access_token: "expired-access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(0),
            email: "alice@example.com".to_string(),
        });
        let encrypted = encrypt_credentials(&crypto, &credentials).unwrap();
        store
            .update_account_sync_state(
                "account-1",
                &serde_json::json!({
                    "credentials": encrypted,
                    "google_client_id": "google-client-id",
                    "google_client_secret": "google-client-secret",
                })
                .to_string(),
            )
            .unwrap();
        let store = Arc::new(store);
        let (ws_tx, _) = broadcast::channel(16);
        let magicpush = MagicPushNotifier::new(store.clone(), crypto.clone()).unwrap();
        let manager = SyncManager::new(
            store,
            crypto,
            PathBuf::new(),
            TEST_SYNC_INTERVAL_SECS,
            ws_tx,
            magicpush,
        );

        manager
            .trigger_sync_with_reason("account-1", SyncTrigger::Startup)
            .await
            .unwrap();

        assert!(manager.handles.lock().await.contains_key("account-1"));
        manager.stop_account_sync("account-1").await;
    }

    #[tokio::test]
    async fn start_account_sync_if_missing_keeps_existing_worker() {
        let (store, crypto) = test_store();
        let store = Arc::new(store);
        let (ws_tx, _) = broadcast::channel(16);
        let magicpush = MagicPushNotifier::new(store.clone(), crypto.clone()).unwrap();
        let manager = SyncManager::new(
            store,
            crypto,
            PathBuf::new(),
            TEST_SYNC_INTERVAL_SECS,
            ws_tx,
            magicpush,
        );
        let (stop_tx, _) = watch::channel(false);
        let (trigger_tx, mut trigger_rx) = mpsc::unbounded_channel();
        let (cancel_tx, _) = watch::channel(false);

        manager.handles.lock().await.insert(
            "account-1".to_string(),
            SyncHandle {
                stop_tx,
                trigger_tx,
                task: tokio::spawn(async {}),
                cancel_tx,
            },
        );

        assert!(!manager
            .start_account_sync_if_missing("account-1")
            .await
            .unwrap());

        manager
            .trigger_sync_with_reason("account-1", SyncTrigger::WindowFocus)
            .await
            .unwrap();
        assert_eq!(trigger_rx.recv().await, Some(SyncTrigger::WindowFocus));
        manager.stop_account_sync("account-1").await;
    }

    #[test]
    fn startup_trigger_is_not_resent_after_worker_start() {
        assert!(!should_resend_trigger_after_worker_start(
            SyncTrigger::Startup
        ));
        assert!(should_resend_trigger_after_worker_start(
            SyncTrigger::Manual
        ));
        assert!(should_resend_trigger_after_worker_start(
            SyncTrigger::WindowFocus
        ));
    }

    #[tokio::test]
    async fn trigger_sync_with_reason_forwards_non_manual_trigger() {
        let (store, crypto) = test_store();
        let store = Arc::new(store);
        let (ws_tx, _) = broadcast::channel(16);
        let magicpush = MagicPushNotifier::new(store.clone(), crypto.clone()).unwrap();
        let manager = SyncManager::new(
            store,
            crypto,
            PathBuf::new(),
            TEST_SYNC_INTERVAL_SECS,
            ws_tx,
            magicpush,
        );
        let (stop_tx, _) = watch::channel(false);
        let (trigger_tx, mut trigger_rx) = mpsc::unbounded_channel();
        let (cancel_tx, _) = watch::channel(false);

        manager.handles.lock().await.insert(
            "account-1".to_string(),
            SyncHandle {
                stop_tx,
                trigger_tx,
                task: tokio::spawn(async {}),
                cancel_tx,
            },
        );

        manager
            .trigger_sync_with_reason("account-1", SyncTrigger::WindowFocus)
            .await
            .unwrap();

        assert_eq!(trigger_rx.recv().await, Some(SyncTrigger::WindowFocus));
        manager.stop_account_sync("account-1").await;
    }

    #[test]
    fn emit_new_mail_event_publishes_message_metadata() {
        let (ws_tx, mut ws_rx) = broadcast::channel::<String>(16);
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        };

        emit_new_mail_event(&ws_tx, &stored);

        let payload: serde_json::Value = serde_json::from_str(&ws_rx.try_recv().unwrap()).unwrap();
        assert_eq!(payload["type"], "new_mail");
        assert_eq!(payload["account_id"], "account-1");
        assert_eq!(payload["message_id"], "msg-1");
        assert_eq!(payload["folder_ids"][0], "inbox");
        assert_eq!(payload["notify"], true);
    }

    #[tokio::test]
    async fn forward_magicpush_message_sends_when_notify_true() {
        let captured = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/api/push", post(capture_request))
            .with_state(captured.clone());
        let (notifier, store) = start_notifier(app).await;
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        };

        forward_magicpush_message(&notifier, store.as_ref(), &stored).await;

        let body = captured.lock().unwrap().clone().unwrap();
        assert!(body.contains("\"title\":\"Deploy report\""));
    }

    #[tokio::test]
    async fn forward_synced_message_emits_new_mail_event_and_magicpush() {
        let captured = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/api/push", post(capture_request))
            .with_state(captured.clone());
        let (notifier, store) = start_notifier(app).await;
        let (ws_tx, mut ws_rx) = broadcast::channel::<String>(16);
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        };

        forward_synced_message(&ws_tx, &notifier, store.as_ref(), &stored).await;

        let ws_payload: serde_json::Value =
            serde_json::from_str(&ws_rx.try_recv().unwrap()).unwrap();
        assert_eq!(ws_payload["type"], "new_mail");
        assert_eq!(ws_payload["notify"], true);
        let body = captured.lock().unwrap().clone().unwrap();
        assert!(body.contains("\"title\":\"Deploy report\""));
    }

    #[tokio::test]
    async fn forward_magicpush_message_skips_when_notify_false() {
        let captured = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/api/push", post(capture_request))
            .with_state(captured.clone());
        let (notifier, store) = start_notifier(app).await;
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: false,
        };

        forward_magicpush_message(&notifier, store.as_ref(), &stored).await;

        assert!(captured.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn forward_magicpush_message_skips_non_inbox_messages() {
        let captured = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/api/push", post(capture_request))
            .with_state(captured.clone());
        let (notifier, store) = start_notifier(app).await;
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["sent".to_string()],
            notify: true,
        };

        forward_magicpush_message(&notifier, store.as_ref(), &stored).await;

        assert!(captured.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn forward_magicpush_message_ignores_send_failure() {
        let app = Router::new().route("/api/push", post(|| async { StatusCode::BAD_GATEWAY }));
        let (notifier, store) = start_notifier(app).await;
        let stored = StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        };

        forward_magicpush_message(&notifier, store.as_ref(), &stored).await;
    }

    fn test_store() -> (Store, Arc<CryptoService>) {
        let store = Store::open_in_memory().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let crypto = Arc::new(CryptoService::init(Some(&temp_dir.path().join("key"))).unwrap());
        let account = Account {
            id: "account-1".to_string(),
            email: "alice@example.com".to_string(),
            display_name: "Alice".to_string(),
            color: None,
            provider: ProviderType::Imap,
            created_at: 1700000000,
            updated_at: 1700000000,
        };
        store.insert_account(&account).unwrap();
        store
            .insert_folder(&test_folder("inbox", FolderRole::Inbox))
            .unwrap();
        store
            .insert_folder(&test_folder("sent", FolderRole::Sent))
            .unwrap();
        store
            .save_app_settings(&AppSettingsRecord {
                id: "active".to_string(),
                public_url: "https://mail.example.com".to_string(),
                created_at: 1700000000,
                updated_at: 1700000000,
            })
            .unwrap();
        let token_encrypted = encrypt_magicpush_token(&crypto, "push-token").unwrap();
        store
            .save_magicpush_config(&MagicPushConfigRecord {
                id: "active".to_string(),
                base_url: "http://127.0.0.1".to_string(),
                token_encrypted: Some(token_encrypted),
                public_url: "https://mail.example.com".to_string(),
                is_enabled: true,
                created_at: 1700000000,
                updated_at: 1700000000,
            })
            .unwrap();
        (store, crypto)
    }

    fn test_folder(id: &str, role: FolderRole) -> Folder {
        Folder {
            id: id.to_string(),
            account_id: "account-1".to_string(),
            remote_id: id.to_uppercase(),
            name: id.to_string(),
            folder_type: FolderType::Folder,
            role: Some(role),
            parent_id: None,
            color: None,
            is_system: true,
            server_linked: true,
            sort_order: 0,
        }
    }

    async fn start_notifier(app: Router) -> (MagicPushNotifier, Arc<Store>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let (store, crypto) = test_store();
        let token_encrypted = encrypt_magicpush_token(&crypto, "push-token").unwrap();
        store
            .save_magicpush_config(&MagicPushConfigRecord {
                id: "active".to_string(),
                base_url: format!("http://{}", addr),
                token_encrypted: Some(token_encrypted),
                public_url: "https://mail.example.com".to_string(),
                is_enabled: true,
                created_at: 1700000000,
                updated_at: 1700000000,
            })
            .unwrap();
        let store = Arc::new(store);
        (
            MagicPushNotifier::new(store.clone(), crypto).unwrap(),
            store,
        )
    }

    async fn capture_request(
        State(captured): State<Arc<Mutex<Option<String>>>>,
        body: Bytes,
    ) -> Json<serde_json::Value> {
        *captured.lock().unwrap() = Some(String::from_utf8(body.to_vec()).unwrap());
        Json(serde_json::json!({
            "success": true,
            "successCount": 1,
            "failedCount": 0
        }))
    }

    fn test_message() -> Message {
        Message {
            id: "msg-1".to_string(),
            account_id: "account-1".to_string(),
            remote_id: "remote-1".to_string(),
            message_id_header: None,
            in_reply_to: None,
            references_header: None,
            thread_id: None,
            subject: "Deploy report".to_string(),
            snippet: "Build finished".to_string(),
            from_address: "alice@example.com".to_string(),
            from_name: "Alice".to_string(),
            to_list: vec![EmailAddress {
                name: None,
                address: "bob@example.com".to_string(),
            }],
            cc_list: Vec::new(),
            bcc_list: Vec::new(),
            body_text: "Build finished with status ok".to_string(),
            body_html_raw: "".to_string(),
            has_attachments: false,
            is_read: false,
            is_starred: false,
            is_draft: false,
            date: 1700000000,
            remote_version: None,
            is_deleted: false,
            deleted_at: None,
            created_at: 1700000000,
            updated_at: 1700000000,
        }
    }
}
