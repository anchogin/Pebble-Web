use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use pebble_core::{now_timestamp, KanbanCard, KanbanColumn};
use serde::Deserialize;
use std::collections::HashMap;

const CONTEXT_NOTES_KEY: &str = "kanban_context_notes";

#[derive(Deserialize)]
pub struct KanbanQuery {
    pub column: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertKanbanRequest {
    pub column: String,
    pub position: Option<i32>,
}

#[derive(Deserialize)]
pub struct SetContextNoteRequest {
    pub note: String,
}

#[derive(Deserialize)]
pub struct MergeContextNotesRequest {
    pub notes: HashMap<String, String>,
}

fn parse_column(value: &str) -> Result<KanbanColumn, ApiError> {
    match value {
        "todo" => Ok(KanbanColumn::Todo),
        "waiting" => Ok(KanbanColumn::Waiting),
        "done" => Ok(KanbanColumn::Done),
        _ => Err(ApiError::BadRequest("Invalid Kanban column".to_string())),
    }
}

fn load_context_notes(state: &AppStateRef) -> Result<HashMap<String, String>, ApiError> {
    let Some(bytes) = state
        .store
        .get_secure_user_data(CONTEXT_NOTES_KEY)
        .map_err(|e| ApiError::Internal(format!("Failed to load context notes: {e}")))?
    else {
        return Ok(HashMap::new());
    };
    serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::Internal(format!("Invalid context notes JSON: {e}")))
}

fn save_context_notes(
    state: &AppStateRef,
    notes: &HashMap<String, String>,
) -> Result<(), ApiError> {
    let bytes = serde_json::to_vec(notes)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize context notes: {e}")))?;
    state
        .store
        .set_secure_user_data(CONTEXT_NOTES_KEY, &bytes)
        .map_err(|e| ApiError::Internal(format!("Failed to save context notes: {e}")))
}

pub async fn list_kanban_cards(
    State(state): State<AppStateRef>,
    Query(query): Query<KanbanQuery>,
) -> Result<Json<Vec<KanbanCard>>, ApiError> {
    let column = query.column.as_deref().map(parse_column).transpose()?;
    let cards = state
        .store
        .list_kanban_cards(column.as_ref())
        .map_err(|e| ApiError::Internal(format!("Failed to list Kanban cards: {e}")))?;

    Ok(Json(cards))
}

pub async fn upsert_kanban_card(
    State(state): State<AppStateRef>,
    Path(message_id): Path<String>,
    Json(body): Json<UpsertKanbanRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = now_timestamp();
    state
        .store
        .upsert_kanban_card(&KanbanCard {
            message_id,
            column: parse_column(&body.column)?,
            position: body.position.unwrap_or(0),
            created_at: now,
            updated_at: now,
        })
        .map_err(|e| ApiError::Internal(format!("Failed to save Kanban card: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_kanban_card(
    State(state): State<AppStateRef>,
    Path(message_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .delete_kanban_card(&message_id)
        .map_err(|e| ApiError::Internal(format!("Failed to delete Kanban card: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_context_notes(
    State(state): State<AppStateRef>,
) -> Result<Json<HashMap<String, String>>, ApiError> {
    Ok(Json(load_context_notes(&state)?))
}

pub async fn set_context_note(
    State(state): State<AppStateRef>,
    Path(message_id): Path<String>,
    Json(body): Json<SetContextNoteRequest>,
) -> Result<Json<HashMap<String, String>>, ApiError> {
    let mut notes = load_context_notes(&state)?;
    if body.note.trim().is_empty() {
        notes.remove(&message_id);
    } else {
        notes.insert(message_id, body.note);
    }
    save_context_notes(&state, &notes)?;
    Ok(Json(notes))
}

pub async fn merge_context_notes(
    State(state): State<AppStateRef>,
    Json(body): Json<MergeContextNotesRequest>,
) -> Result<Json<HashMap<String, String>>, ApiError> {
    let mut notes = load_context_notes(&state)?;
    for (message_id, note) in body.notes {
        if !message_id.trim().is_empty() && !note.trim().is_empty() {
            notes.insert(message_id, note);
        }
    }
    save_context_notes(&state, &notes)?;
    Ok(Json(notes))
}
