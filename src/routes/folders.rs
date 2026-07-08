use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize)]
pub struct FolderResponse {
    pub id: String,
    pub account_id: String,
    pub remote_id: String,
    pub name: String,
    pub role: Option<String>,
    pub unread_count: u32,
    pub folder_type: String,
    pub parent_id: Option<String>,
    pub is_system: bool,
    pub server_linked: bool,
    pub sort_order: i32,
}

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateFolderRequest {
    pub name: String,
}

fn folder_response(folder: pebble_core::Folder, unread_count: u32) -> FolderResponse {
    let folder_type = match folder.folder_type {
        pebble_core::FolderType::Folder => "folder",
        pebble_core::FolderType::Label => "label",
        pebble_core::FolderType::Category => "category",
    };
    FolderResponse {
        id: folder.id,
        account_id: folder.account_id,
        remote_id: folder.remote_id,
        name: folder.name,
        role: folder.role.map(|role| match role {
            pebble_core::FolderRole::Inbox => "inbox".to_string(),
            pebble_core::FolderRole::Sent => "sent".to_string(),
            pebble_core::FolderRole::Drafts => "drafts".to_string(),
            pebble_core::FolderRole::Trash => "trash".to_string(),
            pebble_core::FolderRole::Archive => "archive".to_string(),
            pebble_core::FolderRole::Spam => "spam".to_string(),
        }),
        unread_count,
        folder_type: folder_type.to_string(),
        parent_id: folder.parent_id,
        is_system: folder.is_system,
        server_linked: folder.server_linked,
        sort_order: folder.sort_order,
    }
}

pub async fn create_folder(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
    Json(body): Json<CreateFolderRequest>,
) -> Result<Json<FolderResponse>, ApiError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("Folder name is required".to_string()));
    }
    let folder = state
        .store
        .find_or_create_folder_by_name(&account_id, name, false)
        .map_err(|e| ApiError::Internal(format!("Failed to create folder: {e}")))?;
    Ok(Json(folder_response(folder, 0)))
}

pub async fn list_folders(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<FolderResponse>>, ApiError> {
    let folders = state
        .store
        .list_folders(&account_id)
        .map_err(|e| ApiError::Internal(format!("Failed to list folders: {e}")))?;

    // Get unread counts
    let store2 = state.store.clone();
    let aid2 = account_id.clone();
    let unread_counts: HashMap<String, u32> = store2
        .with_read_async(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT mf.folder_id, COUNT(*)
                 FROM messages m
                 JOIN message_folders mf ON m.id = mf.message_id
                 WHERE m.account_id = ?1 AND m.is_read = 0 AND m.is_deleted = 0
                 GROUP BY mf.folder_id",
            )?;
            let rows = stmt.query_map(rusqlite::params![aid2], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })?;
            let mut counts = HashMap::new();
            for row in rows {
                let (fid, count) = row?;
                counts.insert(fid, count);
            }
            Ok(counts)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get unread counts: {e}")))?;

    let response: Vec<FolderResponse> = folders
        .into_iter()
        .map(|f| {
            let unread_count = unread_counts.get(&f.id).copied().unwrap_or(0);
            folder_response(f, unread_count)
        })
        .collect();

    Ok(Json(response))
}

pub async fn update_folder(
    State(state): State<AppStateRef>,
    Path(folder_id): Path<String>,
    Json(body): Json<UpdateFolderRequest>,
) -> Result<Json<FolderResponse>, ApiError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("Folder name is required".to_string()));
    }
    let folder = state
        .store
        .find_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to load folder: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Folder not found".to_string()))?;
    if folder.is_system || folder.role.is_some() {
        return Err(ApiError::BadRequest(
            "System folders cannot be renamed".to_string(),
        ));
    }
    state
        .store
        .rename_folder(&folder_id, name)
        .map_err(|e| ApiError::Internal(format!("Failed to rename folder: {e}")))?;
    let updated = state
        .store
        .find_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to load folder: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Folder not found".to_string()))?;
    Ok(Json(folder_response(updated, 0)))
}

pub async fn delete_folder(
    State(state): State<AppStateRef>,
    Path(folder_id): Path<String>,
) -> Result<(), ApiError> {
    let folder = state
        .store
        .find_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to load folder: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Folder not found".to_string()))?;
    if folder.is_system || folder.role.is_some() {
        return Err(ApiError::BadRequest(
            "System folders cannot be deleted".to_string(),
        ));
    }
    state
        .store
        .delete_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to delete folder: {e}")))?;
    Ok(())
}

pub async fn link_folder(
    State(state): State<AppStateRef>,
    Path(folder_id): Path<String>,
) -> Result<Json<FolderResponse>, ApiError> {
    set_folder_link_state(state, folder_id, true)
}

pub async fn unlink_folder(
    State(state): State<AppStateRef>,
    Path(folder_id): Path<String>,
) -> Result<Json<FolderResponse>, ApiError> {
    set_folder_link_state(state, folder_id, false)
}

fn set_folder_link_state(
    state: AppStateRef,
    folder_id: String,
    server_linked: bool,
) -> Result<Json<FolderResponse>, ApiError> {
    state
        .store
        .find_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to load folder: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Folder not found".to_string()))?;
    state
        .store
        .set_folder_server_linked(&folder_id, server_linked)
        .map_err(|e| ApiError::Internal(format!("Failed to update folder link: {e}")))?;
    let updated = state
        .store
        .find_folder_by_id(&folder_id)
        .map_err(|e| ApiError::Internal(format!("Failed to load folder: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Folder not found".to_string()))?;
    Ok(Json(folder_response(updated, 0)))
}

pub async fn get_folder_unread_counts(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<HashMap<String, u32>>, ApiError> {
    let store = state.store.clone();

    let counts = store
        .with_read_async(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT f.id, COUNT(CASE WHEN m.is_read = 0 THEN 1 END) as unread
                 FROM folders f
                 LEFT JOIN message_folders mf ON f.id = mf.folder_id
                 LEFT JOIN messages m ON mf.message_id = m.id AND m.is_deleted = 0
                 WHERE f.account_id = ?1
                 GROUP BY f.id",
            )?;
            let rows = stmt.query_map(rusqlite::params![account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })?;
            let mut counts = HashMap::new();
            for row in rows {
                let (fid, count) = row?;
                counts.insert(fid, count);
            }
            Ok(counts)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get folder unread counts: {e}")))?;

    Ok(Json(counts))
}
