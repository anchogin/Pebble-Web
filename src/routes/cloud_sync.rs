use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{extract::State, Json};
use pebble_store::cloud_sync::{preview_backup, WebDavClient, SETTINGS_BACKUP_FILENAME};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WebDavRequest {
    pub url: String,
    pub username: String,
    pub password: String,
}

fn webdav_client(body: WebDavRequest) -> Result<WebDavClient, ApiError> {
    WebDavClient::new(body.url, body.username, body.password)
        .map_err(|e| ApiError::BadRequest(format!("{e}")))
}

pub async fn test_webdav_connection(
    Json(body): Json<WebDavRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    webdav_client(body)?
        .test_connection()
        .await
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;
    Ok(Json(serde_json::json!({ "ok": true, "result": "ok" })))
}

pub async fn backup_to_webdav(
    State(state): State<AppStateRef>,
    Json(body): Json<WebDavRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let data = state
        .store
        .export_settings()
        .map_err(|e| ApiError::Internal(format!("Failed to export settings: {e}")))?;
    webdav_client(body)?
        .upload(SETTINGS_BACKUP_FILENAME, &data)
        .await
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;

    Ok(Json(serde_json::json!({ "ok": true, "result": "ok" })))
}

pub async fn preview_webdav_backup(
    Json(body): Json<WebDavRequest>,
) -> Result<Json<pebble_store::cloud_sync::BackupPreview>, ApiError> {
    let data = webdav_client(body)?
        .download(SETTINGS_BACKUP_FILENAME)
        .await
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;
    let preview = preview_backup(&data).map_err(|e| ApiError::BadRequest(format!("{e}")))?;
    Ok(Json(preview))
}

pub async fn restore_from_webdav(
    State(state): State<AppStateRef>,
    Json(body): Json<WebDavRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let data = webdav_client(body)?
        .download(SETTINGS_BACKUP_FILENAME)
        .await
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;
    state
        .store
        .import_settings(&data)
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;

    Ok(Json(serde_json::json!({ "ok": true, "result": "ok" })))
}
