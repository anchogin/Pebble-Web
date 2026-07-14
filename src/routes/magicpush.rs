use crate::error::ApiError;
use crate::magicpush::{
    decrypt_magicpush_token, encrypt_magicpush_token, resolve_magicpush_config_record,
    shared_public_url, MagicPushNotifier, ResolvedMagicPushConfig,
};
use crate::state::AppStateRef;
use axum::{extract::State, Json};
use pebble_core::{now_timestamp, MagicPushConfigRecord};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagicPushConfigResponse {
    pub enabled: bool,
    pub base_url: String,
    pub has_token: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMagicPushConfigRequest {
    pub enabled: bool,
    pub base_url: String,
    pub token: Option<String>,
    #[serde(default)]
    pub clear_token: bool,
}

pub async fn get_magicpush_config(
    State(state): State<AppStateRef>,
) -> Result<Json<MagicPushConfigResponse>, ApiError> {
    let config = state
        .store
        .get_magicpush_config()
        .map_err(|e| ApiError::Internal(format!("Failed to load MagicPush config: {e}")))?;

    Ok(Json(match config {
        Some(config) => MagicPushConfigResponse {
            enabled: config.is_enabled,
            base_url: config.base_url,
            has_token: config
                .token_encrypted
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
        },
        None => MagicPushConfigResponse {
            enabled: false,
            base_url: String::new(),
            has_token: false,
        },
    }))
}

pub async fn save_magicpush_config(
    State(state): State<AppStateRef>,
    Json(body): Json<SaveMagicPushConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let existing = state
        .store
        .get_magicpush_config()
        .map_err(|e| ApiError::Internal(format!("Failed to load MagicPush config: {e}")))?;
    let record = build_record(&state, body, existing.as_ref())?;
    state
        .store
        .save_magicpush_config(&record)
        .map_err(|e| ApiError::Internal(format!("Failed to save MagicPush config: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn test_magicpush_connection(
    State(state): State<AppStateRef>,
    Json(body): Json<SaveMagicPushConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let existing = state
        .store
        .get_magicpush_config()
        .map_err(|e| ApiError::Internal(format!("Failed to load MagicPush config: {e}")))?;
    let record = build_record(&state, body, existing.as_ref())?;
    let token = record
        .token_encrypted
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("MagicPush token is required".to_string()))
        .and_then(|encrypted| {
            decrypt_magicpush_token(&state.crypto, encrypted)
                .map_err(|e| ApiError::BadRequest(format!("Invalid MagicPush token: {e}")))
        })?;
    let public_url = shared_public_url(state.store.as_ref())
        .map_err(|e| ApiError::Internal(format!("Failed to load app settings: {e}")))?
        .ok_or_else(|| ApiError::BadRequest("Pebble public URL is required".to_string()))?;
    let resolved =
        resolve_magicpush_config_record(&record, public_url, token).ok_or_else(|| {
            ApiError::BadRequest(
                "MagicPush base URL, token, and public URL are required".to_string(),
            )
        })?;
    let notifier = MagicPushNotifier::new(state.store.clone(), state.crypto.clone())
        .map_err(ApiError::Internal)?;
    notifier
        .send_test_push(&ResolvedMagicPushConfig { ..resolved })
        .await
        .map_err(|e| ApiError::BadRequest(format!("MagicPush test failed: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

fn build_record(
    state: &AppStateRef,
    body: SaveMagicPushConfigRequest,
    existing: Option<&MagicPushConfigRecord>,
) -> Result<MagicPushConfigRecord, ApiError> {
    let base_url = trim_trailing_slash(&body.base_url);
    let token = body
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let token_encrypted = if body.clear_token {
        token
            .map(|token| encrypt_magicpush_token(&state.crypto, token))
            .transpose()
            .map_err(ApiError::Internal)?
    } else if let Some(token) = token {
        Some(encrypt_magicpush_token(&state.crypto, token).map_err(ApiError::Internal)?)
    } else {
        existing.and_then(|config| config.token_encrypted.clone())
    };

    if body.enabled {
        if base_url.is_empty() {
            return Err(ApiError::BadRequest(
                "MagicPush base URL is required".to_string(),
            ));
        }
        let public_url = shared_public_url(state.store.as_ref())
            .map_err(|e| ApiError::Internal(format!("Failed to load app settings: {e}")))?;
        if public_url.is_none() {
            return Err(ApiError::BadRequest(
                "Pebble public URL is required".to_string(),
            ));
        }
        if token_encrypted.as_deref().is_none_or(str::is_empty) {
            return Err(ApiError::BadRequest(
                "MagicPush token is required".to_string(),
            ));
        }
    }

    let now = now_timestamp();
    Ok(MagicPushConfigRecord {
        id: "active".to_string(),
        base_url,
        token_encrypted,
        public_url: String::new(),
        is_enabled: body.enabled,
        created_at: existing.map_or(now, |config| config.created_at),
        updated_at: now,
    })
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn save_enabled_requires_complete_config() {
        let state = test_state_ref();

        let err = build_record(
            &state,
            SaveMagicPushConfigRequest {
                enabled: true,
                base_url: "".to_string(),
                token: Some("token".to_string()),
                clear_token: false,
            },
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("base URL"));
    }

    #[test]
    fn save_without_token_preserves_existing_token() {
        let state = test_state_ref();
        state
            .store
            .save_app_settings(&pebble_core::AppSettingsRecord {
                id: "active".to_string(),
                public_url: "https://mail.example.com".to_string(),
                created_at: 1,
                updated_at: 1,
            })
            .unwrap();
        let existing = MagicPushConfigRecord {
            id: "active".to_string(),
            base_url: "https://old.example.com".to_string(),
            token_encrypted: Some("encrypted-token".to_string()),
            public_url: "https://old-mail.example.com".to_string(),
            is_enabled: true,
            created_at: 1,
            updated_at: 1,
        };

        let record = build_record(
            &state,
            SaveMagicPushConfigRequest {
                enabled: true,
                base_url: "https://push.example.com/".to_string(),
                token: None,
                clear_token: false,
            },
            Some(&existing),
        )
        .unwrap();

        assert_eq!(record.base_url, "https://push.example.com");
        assert_eq!(record.public_url, "");
        assert_eq!(record.token_encrypted.as_deref(), Some("encrypted-token"));
    }

    #[test]
    fn save_enabled_uses_shared_public_url_requirement() {
        let state = test_state_ref();
        state
            .store
            .save_app_settings(&pebble_core::AppSettingsRecord {
                id: "active".to_string(),
                public_url: "https://mail.example.com".to_string(),
                created_at: 1,
                updated_at: 1,
            })
            .unwrap();

        let record = build_record(
            &state,
            SaveMagicPushConfigRequest {
                enabled: true,
                base_url: "https://push.example.com".to_string(),
                token: Some("token".to_string()),
                clear_token: false,
            },
            None,
        )
        .unwrap();

        assert_eq!(record.public_url, "");
    }

    #[tokio::test]
    async fn test_push_surfaces_magicpush_400_json_message() {
        let app = axum::Router::new().route(
            "/api/push",
            axum::routing::post(|| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "success": false,
                        "code": 400,
                        "message": "该接口未绑定任何渠道"
                    })),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let state = test_state_ref();
        state
            .store
            .save_app_settings(&pebble_core::AppSettingsRecord {
                id: "active".to_string(),
                public_url: "https://mail.example.com".to_string(),
                created_at: 1,
                updated_at: 1,
            })
            .unwrap();

        let error = test_magicpush_connection(
            axum::extract::State(state),
            Json(SaveMagicPushConfigRequest {
                enabled: true,
                base_url: format!("http://{addr}"),
                token: Some("push-token".to_string()),
                clear_token: false,
            }),
        )
        .await
        .unwrap_err();

        assert!(
            error.to_string().contains("该接口未绑定任何渠道"),
            "expected MagicPush response body message, got: {error}"
        );
    }

    pub(crate) fn test_state_ref() -> AppStateRef {
        use crate::config::Config;
        use crate::sync::SyncManager;
        use pebble_crypto::CryptoService;
        use pebble_store::Store;
        use std::path::PathBuf;
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::{broadcast, Mutex};

        const TEST_SYNC_INTERVAL_SECS: u64 = 300;

        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(Store::open_in_memory().unwrap());
        let crypto = Arc::new(CryptoService::init(Some(&temp_dir.path().join("key"))).unwrap());
        let (ws_broadcast, _) = broadcast::channel(1);
        let sync_manager = Arc::new(SyncManager::new(
            store.clone(),
            crypto.clone(),
            PathBuf::new(),
            TEST_SYNC_INTERVAL_SECS,
            ws_broadcast.clone(),
            MagicPushNotifier::new(store.clone(), crypto.clone()).unwrap(),
        ));
        Arc::new(crate::state::AppState {
            config: Config {
                port: 8080,
                data_dir: temp_dir.path().to_path_buf(),
                password_hash: "hash".to_string(),
                jwt_secret: "secretsecretsecretsecretsecretsecret12".to_string(),
                sync_interval_secs: TEST_SYNC_INTERVAL_SECS,
                log_retain_days: 7,
                google_client_id: None,
                google_client_secret: None,
                public_url: None,
            },
            store,
            crypto,
            attachments_dir: PathBuf::new(),
            sync_manager,
            ws_broadcast,
            oauth_sessions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            rule_runs: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }
}
