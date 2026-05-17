use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use pebble_core::{new_id, now_timestamp, Rule};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub priority: i32,
    pub conditions: String,
    pub actions: String,
    pub is_enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateRuleRequest {
    pub name: String,
    pub priority: i32,
    pub conditions: String,
    pub actions: String,
    pub is_enabled: bool,
}

pub async fn list_rules(State(state): State<AppStateRef>) -> Result<Json<Vec<Rule>>, ApiError> {
    let rules = state
        .store
        .list_rules()
        .map_err(|e| ApiError::Internal(format!("Failed to list rules: {e}")))?;

    Ok(Json(rules))
}

pub async fn create_rule(
    State(state): State<AppStateRef>,
    Json(body): Json<CreateRuleRequest>,
) -> Result<Json<Rule>, ApiError> {
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("Rule name is required".to_string()));
    }
    let now = now_timestamp();
    let rule = Rule {
        id: new_id(),
        name: body.name,
        priority: body.priority,
        conditions: body.conditions,
        actions: body.actions,
        is_enabled: body.is_enabled.unwrap_or(true),
        created_at: now,
        updated_at: now,
    };
    state
        .store
        .insert_rule(&rule)
        .map_err(|e| ApiError::Internal(format!("Failed to create rule: {e}")))?;

    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRuleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let existing = state
        .store
        .list_rules()
        .map_err(|e| ApiError::Internal(format!("Failed to load rules: {e}")))?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| ApiError::NotFound("Rule not found".to_string()))?;

    state
        .store
        .update_rule(&Rule {
            id,
            name: body.name,
            priority: body.priority,
            conditions: body.conditions,
            actions: body.actions,
            is_enabled: body.is_enabled,
            created_at: existing.created_at,
            updated_at: now_timestamp(),
        })
        .map_err(|e| ApiError::Internal(format!("Failed to update rule: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_rule(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .delete_rule(&id)
        .map_err(|e| ApiError::Internal(format!("Failed to delete rule: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
