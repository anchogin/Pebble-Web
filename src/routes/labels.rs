use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use pebble_store::labels::Label;
use rusqlite::OptionalExtension;
use serde::Deserialize;

pub async fn list_labels(State(state): State<AppStateRef>) -> Result<Json<Vec<Label>>, ApiError> {
    let labels = state
        .store
        .list_labels()
        .map_err(|e| ApiError::Internal(format!("Failed to list labels: {e}")))?;

    Ok(Json(labels))
}

#[derive(Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: Option<String>,
}

pub async fn create_label(
    State(state): State<AppStateRef>,
    Json(body): Json<CreateLabelRequest>,
) -> Result<Json<Label>, ApiError> {
    let store = state.store.clone();
    let name = body.name;
    let color = body.color.unwrap_or_else(|| "#808080".to_string());

    let label = store
        .with_write_async(move |conn| {
            let id = pebble_core::new_id();
            conn.execute(
                "INSERT INTO labels (id, name, color, is_system) VALUES (?1, ?2, ?3, 0)",
                rusqlite::params![id, name, color],
            )?;
            Ok(Label {
                id,
                name,
                color,
                is_system: false,
                rule_id: None,
            })
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create label: {e}")))?;

    Ok(Json(label))
}

pub async fn delete_label(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();

    store
        .with_write_async(move |conn| {
            conn.execute(
                "DELETE FROM message_labels WHERE label_id = ?1",
                rusqlite::params![id],
            )?;
            conn.execute("DELETE FROM labels WHERE id = ?1", rusqlite::params![id])?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete label: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchLabelRequest {
    pub message_ids: Vec<String>,
}

pub async fn get_message_labels(
    State(state): State<AppStateRef>,
    Path(message_id): Path<String>,
) -> Result<Json<Vec<Label>>, ApiError> {
    let labels = state
        .store
        .get_message_labels(&message_id)
        .map_err(|e| ApiError::Internal(format!("Failed to get message labels: {e}")))?;

    Ok(Json(labels))
}

pub async fn get_message_labels_batch(
    State(state): State<AppStateRef>,
    Json(body): Json<BatchLabelRequest>,
) -> Result<Json<std::collections::HashMap<String, Vec<Label>>>, ApiError> {
    let labels = state
        .store
        .get_message_labels_batch(&body.message_ids)
        .map_err(|e| ApiError::Internal(format!("Failed to get message labels: {e}")))?;

    Ok(Json(labels))
}

#[derive(Deserialize)]
pub struct AddLabelToMessageRequest {
    pub label_id: Option<String>,
    pub label_name: Option<String>,
}

pub async fn add_label_to_message(
    State(state): State<AppStateRef>,
    Path(message_id): Path<String>,
    Json(body): Json<AddLabelToMessageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();

    let label_id = if let Some(id) = body.label_id {
        id
    } else if let Some(name) = body.label_name {
        let store_ref = store.clone();
        store_ref
            .with_read_async(move |conn| {
                let id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM labels WHERE name = ?1",
                        rusqlite::params![name],
                        |row| row.get(0),
                    )
                    .optional()?;
                id.ok_or_else(|| {
                    pebble_core::PebbleError::Storage(format!("Label not found: {name}"))
                })
            })
            .await
            .map_err(|e| ApiError::NotFound(format!("Label not found: {e}")))?
    } else {
        return Err(ApiError::BadRequest(
            "label_id or label_name is required".to_string(),
        ));
    };

    store
        .with_write_async(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO message_labels (message_id, label_id) VALUES (?1, ?2)",
                rusqlite::params![message_id, label_id],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add label to message: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_label_from_message(
    State(state): State<AppStateRef>,
    Path((message_id, label_id_or_name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();
    let mid = message_id.clone();
    let label_param = label_id_or_name.clone();

    // First try to delete using the parameter as a label_id directly
    let rows_affected = store
        .with_write_async(move |conn| {
            let affected = conn.execute(
                "DELETE FROM message_labels WHERE message_id = ?1 AND label_id = ?2",
                rusqlite::params![mid, label_param],
            )?;
            Ok(affected)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove label from message: {e}")))?;

    if rows_affected == 0 {
        // No rows affected — try looking up the label by name
        let store2 = state.store.clone();
        let label_name = label_id_or_name.clone();

        let found_label_id: Option<String> = store2
            .with_read_async(move |conn| {
                let id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM labels WHERE name = ?1",
                        rusqlite::params![label_name],
                        |row| row.get(0),
                    )
                    .optional()?;
                Ok(id)
            })
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to lookup label by name: {e}")))?;

        if let Some(resolved_id) = found_label_id {
            let store3 = state.store.clone();
            let mid2 = message_id.clone();
            store3
                .with_write_async(move |conn| {
                    conn.execute(
                        "DELETE FROM message_labels WHERE message_id = ?1 AND label_id = ?2",
                        rusqlite::params![mid2, resolved_id],
                    )?;
                    Ok(())
                })
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("Failed to remove label from message: {e}"))
                })?;
        }
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
