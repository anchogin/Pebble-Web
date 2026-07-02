use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pebble_crypto::CryptoService;
use pebble_mail::{gmail_sync::GmailSyncWorker, provider::gmail::GmailProvider, ConnectionSecurity, ImapConfig, ImapMailProvider, SyncConfig, SyncWorker, SyncTrigger};
use pebble_oauth::{OAuthConfig, OAuthManager, OAuthNetworkConfig};
use pebble_store::Store;
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::credentials::{decrypt_credentials, AccountCredentials, GmailCredentials, ImapCredentials};

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
}

impl SyncManager {
    pub fn new(
        store: Arc<Store>,
        crypto: Arc<CryptoService>,
        attachments_dir: PathBuf,
        sync_interval_secs: u64,
        ws_tx: broadcast::Sender<String>,
    ) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            store,
            crypto,
            attachments_dir,
            sync_interval_secs,
            ws_tx,
        }
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
        let mut handles = self.handles.lock().await;

        // If already running, stop the existing worker first.
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
                self.start_imap_worker(account_id, imap.clone(), sync_state, handles).await
            }
            AccountCredentials::Gmail(gmail_creds) => {
                self.start_gmail_worker(account_id, gmail_creds, sync_state, handles).await
            }
        }
    }

    async fn start_imap_worker(
        &self,
        account_id: &str,
        imap_creds: ImapCredentials,
        sync_state: serde_json::Value,
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

        let sync_config = SyncConfig {
            poll_interval_secs: self.sync_interval_secs,
            ..SyncConfig::default()
        };

        let account_id_owned = account_id.to_string();
        let ws_tx = self.ws_tx.clone();
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
                trigger_tx: trigger_tx.clone(), // Clone trigger_tx before moving it
                task,
                cancel_tx,
            },
        );
        info!("[SyncManager] Gmail worker handle inserted for account: {}", account_id);

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
        info!("[SyncManager] Starting Gmail worker for account: {}", account_id);

        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<pebble_mail::SyncProgress>();
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<pebble_mail::SyncLogEntry>();

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
        .with_log_tx(log_tx.clone());

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
                                                state["credentials"] =
                                                    serde_json::json!(new_enc);
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

        let trigger_tx_clone = trigger_tx.clone();
        let account_id_clone = account_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            info!("[SyncManager] Sending initial Startup trigger for account: {}", account_id_clone);
            if trigger_tx_clone.send(SyncTrigger::Startup).is_err() {
                error!("Failed to send initial Full sync trigger for {}", account_id_clone);
            }
        });

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
        info!("[SyncManager] Received trigger for account: {}", account_id);
        let handles = self.handles.lock().await;
        if let Some(handle) = handles.get(account_id) {
            info!("[SyncManager] Found handle, sending SyncTrigger::Manual...");
            let result = handle.trigger_tx.send(SyncTrigger::Manual);
            if let Err(e) = result {
                error!("[SyncManager] Failed to send trigger: {}. Channel may be closed.", e);
                return Err("Sync worker channel closed".to_string());
            } else {
                info!("[SyncManager] Trigger sent successfully.");
            }
        } else {
            error!("[SyncManager] No sync worker handle found for account {}", account_id);
            return Err(format!("No sync worker running for account {account_id}"));
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
    pub fn get_sync_worker(&self, account_id: &str) -> Option<SyncWorkerHandle> {
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
