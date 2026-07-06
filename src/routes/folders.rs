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
    pub name: String,
    pub role: Option<String>,
    pub unread_count: u32,
    pub folder_type: String,
    pub parent_id: Option<String>,
    pub is_system: bool,
    pub sort_order: i32,
}

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
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
    let folder_type = match folder.folder_type {
        pebble_core::FolderType::Folder => "folder",
        pebble_core::FolderType::Label => "label",
        pebble_core::FolderType::Category => "category",
    };
    Ok(Json(FolderResponse {
        id: folder.id,
        account_id: folder.account_id,
        name: folder.name,
        role: folder.role.map(|role| match role {
            pebble_core::FolderRole::Inbox => "inbox".to_string(),
            pebble_core::FolderRole::Sent => "sent".to_string(),
            pebble_core::FolderRole::Drafts => "drafts".to_string(),
            pebble_core::FolderRole::Trash => "trash".to_string(),
            pebble_core::FolderRole::Archive => "archive".to_string(),
            pebble_core::FolderRole::Spam => "spam".to_string(),
        }),
        unread_count: 0,
        folder_type: folder_type.to_string(),
        parent_id: folder.parent_id,
        is_system: folder.is_system,
        sort_order: folder.sort_order,
    }))
}

pub async fn list_folders(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<FolderResponse>>, ApiError> {
    let store = state.store.clone();
    let aid = account_id.clone();

    let folders = store
        .with_read_async(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, account_id, remote_id, name, folder_type, role, parent_id, color, is_system, sort_order
                 FROM folders WHERE account_id = ?1 ORDER BY sort_order ASC",
            )?;
            let rows = stmt.query_map(rusqlite::params![aid], |row| {
                let role_str: Option<String> = row.get(5)?;
                let is_system: i32 = row.get(8)?;
                Ok(pebble_core::Folder {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    remote_id: row.get(2)?,
                    name: row.get(3)?,
                    folder_type: match row.get::<_, String>(4)?.as_str() {
                        "label" => pebble_core::FolderType::Label,
                        "category" => pebble_core::FolderType::Category,
                        _ => pebble_core::FolderType::Folder,
                    },
                    role: role_str.and_then(|s| match s.as_str() {
                        "inbox" => Some(pebble_core::FolderRole::Inbox),
                        "sent" => Some(pebble_core::FolderRole::Sent),
                        "drafts" => Some(pebble_core::FolderRole::Drafts),
                        "trash" => Some(pebble_core::FolderRole::Trash),
                        "archive" => Some(pebble_core::FolderRole::Archive),
                        "spam" => Some(pebble_core::FolderRole::Spam),
                        _ => None,
                    }),
                    parent_id: row.get(6)?,
                    color: row.get(7)?,
                    is_system: is_system != 0,
                    sort_order: row.get(9)?,
                })
            })?;
            let mut folders = Vec::new();
            for row in rows {
                folders.push(row?);
            }
            Ok(folders)
        })
        .await
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
            let role = f.role.map(|r| match r {
                pebble_core::FolderRole::Inbox => "inbox".to_string(),
                pebble_core::FolderRole::Sent => "sent".to_string(),
                pebble_core::FolderRole::Drafts => "drafts".to_string(),
                pebble_core::FolderRole::Trash => "trash".to_string(),
                pebble_core::FolderRole::Archive => "archive".to_string(),
                pebble_core::FolderRole::Spam => "spam".to_string(),
            });
            let folder_type = match f.folder_type {
                pebble_core::FolderType::Folder => "folder",
                pebble_core::FolderType::Label => "label",
                pebble_core::FolderType::Category => "category",
            };
            let unread_count = unread_counts.get(&f.id).copied().unwrap_or(0);
            FolderResponse {
                id: f.id,
                account_id: f.account_id,
                name: f.name,
                role,
                unread_count,
                folder_type: folder_type.to_string(),
                parent_id: f.parent_id,
                is_system: f.is_system,
                sort_order: f.sort_order,
            }
        })
        .collect();

    Ok(Json(response))
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
