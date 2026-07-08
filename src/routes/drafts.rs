use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use pebble_core::{
    new_id, now_timestamp, Attachment, EmailAddress, Folder, FolderRole, FolderType, Message,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftRequest {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub existing_draft_id: Option<String>,
    pub attachment_paths: Option<Vec<String>>,
}

fn address_list(values: Vec<String>) -> Vec<EmailAddress> {
    values
        .into_iter()
        .map(|value| EmailAddress {
            name: None,
            address: value,
        })
        .collect()
}

fn attachment_for_path(message_id: &str, path: &str) -> Attachment {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment")
        .to_string();
    Attachment {
        id: new_id(),
        message_id: message_id.to_string(),
        filename,
        mime_type: "application/octet-stream".to_string(),
        size: std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0),
        local_path: Some(path.to_string()),
        content_id: None,
        is_inline: false,
    }
}

pub async fn save_draft(
    State(state): State<AppStateRef>,
    Json(body): Json<SaveDraftRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();
    let account_id = body.account_id.clone();
    let draft_id = body.existing_draft_id.clone().unwrap_or_else(new_id);
    let now = now_timestamp();
    let attachment_paths = body.attachment_paths.unwrap_or_default();

    let folder_id = store
        .with_write_async(move |conn| {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM folders WHERE account_id = ?1 AND role = 'drafts'",
                    rusqlite::params![account_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                return Ok(id);
            }

            let folder = Folder {
                id: new_id(),
                account_id,
                remote_id: "__local_drafts__".to_string(),
                name: "Drafts".to_string(),
                folder_type: FolderType::Folder,
                role: Some(FolderRole::Drafts),
                parent_id: None,
                color: None,
                is_system: true,
                server_linked: false,
                sort_order: 2,
            };
            conn.execute(
                "INSERT INTO folders (id, account_id, remote_id, name, folder_type, role, parent_id, color, is_system, server_linked, sort_order)
                 VALUES (?1, ?2, ?3, ?4, 'folder', 'drafts', NULL, NULL, 1, 0, ?5)",
                rusqlite::params![folder.id, folder.account_id, folder.remote_id, folder.name, folder.sort_order],
            )?;
            Ok(folder.id)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to prepare drafts folder: {e}")))?;

    let message = Message {
        id: draft_id.clone(),
        account_id: body.account_id,
        remote_id: draft_id.clone(),
        message_id_header: None,
        in_reply_to: body.in_reply_to,
        references_header: None,
        thread_id: None,
        subject: body.subject,
        snippet: body.body_text.chars().take(180).collect(),
        from_address: String::new(),
        from_name: String::new(),
        to_list: address_list(body.to),
        cc_list: address_list(body.cc),
        bcc_list: address_list(body.bcc),
        body_text: body.body_text,
        body_html_raw: body.body_html.unwrap_or_default(),
        has_attachments: !attachment_paths.is_empty(),
        is_read: true,
        is_starred: false,
        is_draft: true,
        date: now,
        remote_version: None,
        is_deleted: false,
        deleted_at: None,
        created_at: now,
        updated_at: now,
    };
    let attachments: Vec<Attachment> = attachment_paths
        .iter()
        .map(|path| attachment_for_path(&draft_id, path))
        .collect();

    state
        .store
        .replace_message_with_attachments(&message, &[folder_id], &attachments)
        .map_err(|e| ApiError::Internal(format!("Failed to save draft: {e}")))?;

    Ok(Json(serde_json::json!({ "id": draft_id })))
}

pub async fn delete_draft(
    State(state): State<AppStateRef>,
    Path((account_id, draft_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .with_write_async(move |conn| {
            conn.execute(
                "DELETE FROM messages WHERE id = ?1 AND account_id = ?2 AND is_draft = 1",
                rusqlite::params![draft_id, account_id],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete draft: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

use rusqlite::OptionalExtension;
