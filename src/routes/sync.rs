use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerSyncRequest {
    pub account_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn trigger_sync(
    State(state): State<AppStateRef>,
    Json(body): Json<TriggerSyncRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .sync_manager
        .trigger_sync(&body.account_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to trigger sync: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSyncConfigRequest {
    pub sync_strategy: String,
    #[serde(default)]
    pub sync_since_date: Option<String>,
}

pub async fn update_sync_config(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
    Json(body): Json<UpdateSyncConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let strategy = match body.sync_strategy.as_str() {
        "all" | "since_date" | "recent" => body.sync_strategy.clone(),
        other => {
            return Err(ApiError::BadRequest(format!(
                "Unknown sync strategy: {other}"
            )));
        }
    };

    let since_date = if strategy == "since_date" {
        body.sync_since_date
    } else {
        None
    };

    state
        .store
        .update_sync_state(&account_id, |s| {
            s.sync_strategy = Some(strategy);
            s.sync_since_date = since_date;
        })
        .map_err(|e| ApiError::Internal(format!("Failed to update sync config: {e}")))?;

    // Restart the sync worker so the new strategy takes effect.
    let sync_manager = state.sync_manager.clone();
    let aid = account_id.clone();
    tokio::spawn(async move {
        sync_manager.stop_account_sync(&aid).await;
        if let Err(e) = sync_manager.start_account_sync(&aid).await {
            tracing::warn!("Failed to restart sync after config update: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let _ = sync_manager.trigger_sync(&aid).await;
    });

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Sync settings saved. Restarting sync..."
    })))
}
