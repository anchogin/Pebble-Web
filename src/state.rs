use crate::config::Config;
use crate::sync::SyncManager;
use pebble_crypto::CryptoService;
use pebble_rules::RunControl;
use pebble_store::Store;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub type AppStateRef = Arc<AppState>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuleRunKey {
    Single(String),
    All,
}

#[derive(Debug, Clone)]
pub struct ActiveRuleRun {
    pub run_id: String,
    pub control: Arc<RunControl>,
}

#[derive(Debug, Clone)]
pub enum OAuthSessionResult {
    Pending,
    Complete { account_id: String, email: String },
    Error(String),
}

pub struct OAuthSession {
    pub pkce_state: Option<pebble_oauth::PkceState>,
    pub display_name: String,
    pub result: OAuthSessionResult,
}

pub struct AppState {
    pub config: Config,
    pub store: Arc<Store>,
    pub crypto: Arc<CryptoService>,
    pub attachments_dir: PathBuf,
    pub sync_manager: Arc<SyncManager>,
    pub ws_broadcast: broadcast::Sender<String>,
    pub oauth_sessions: Arc<Mutex<HashMap<String, OAuthSession>>>,
    pub rule_runs: Arc<Mutex<HashMap<RuleRunKey, ActiveRuleRun>>>,
}

impl AppState {
    pub fn init(config: Config) -> Result<Self, String> {
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|e| format!("Failed to create data dir: {e}"))?;
        std::fs::create_dir_all(config.attachments_dir())
            .map_err(|e| format!("Failed to create attachments dir: {e}"))?;

        let store =
            Store::open(&config.db_path()).map_err(|e| format!("Failed to open store: {e}"))?;

        let key_file = config.key_file_path();
        let crypto = CryptoService::init(Some(&key_file))
            .map_err(|e| format!("Failed to init crypto: {e}"))?;

        let attachments_dir = config.attachments_dir();

        let store = Arc::new(store);
        let crypto = Arc::new(crypto);

        let (ws_broadcast, _) = broadcast::channel(100);

        let sync_manager = Arc::new(SyncManager::new(
            store.clone(),
            crypto.clone(),
            attachments_dir.clone(),
            config.sync_interval_secs,
            ws_broadcast.clone(),
        ));

        Ok(Self {
            config,
            store,
            crypto,
            attachments_dir,
            sync_manager,
            ws_broadcast,
            oauth_sessions: Arc::new(Mutex::new(HashMap::new())),
            rule_runs: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
