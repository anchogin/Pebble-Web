use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use pebble_core::{new_id, now_timestamp, Rule};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub priority: i32,
    pub conditions: String,
    pub actions: String,
    pub is_enabled: Option<bool>,
    pub account_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRuleRequest {
    pub name: String,
    pub priority: i32,
    pub conditions: String,
    pub actions: String,
    pub is_enabled: bool,
    pub account_id: Option<String>,
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
        account_id: body.account_id,
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
            account_id: body.account_id,
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

struct WsRuleSink {
    rule_id: Option<String>,
    tx: broadcast::Sender<String>,
}

impl pebble_rules::ProgressSink for WsRuleSink {
    fn emit(&self, ev: pebble_rules::RuleProgressEvent) {
        let mut value = serde_json::to_value(&ev).unwrap_or_else(|_| json!({}));
        if let Some(obj) = value.as_object_mut() {
            obj.insert("rule_id".to_string(), json!(self.rule_id.as_ref()));
        }
        let phase = value
            .get("phase")
            .and_then(|phase| phase.as_str())
            .unwrap_or("error")
            .to_lowercase();
        let wrap = json!({
            "type": format!("rules_exec_{phase}"),
            "rule_id": self.rule_id.as_ref(),
            "data": value,
        });
        let _ = self.tx.send(wrap.to_string());
    }
}

pub async fn execute_rule(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rule = state
        .store
        .list_all_rules()
        .map_err(|e| ApiError::Internal(format!("Failed to list rules: {e}")))?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| ApiError::NotFound("Rule not found".to_string()))?;

    let rule_id = rule.id.clone();
    let store = state.store.clone();
    let tx = state.ws_broadcast.clone();
    tokio::task::spawn_blocking(move || {
        let sink = WsRuleSink {
            rule_id: Some(rule_id.clone()),
            tx: tx.clone(),
        };
        if let Err(e) = pebble_rules::run_single_rule(&rule, store.as_ref(), Some(&sink)) {
            let _ = tx.send(
                json!({
                    "type": "rules_exec_error",
                    "rule_id": rule_id,
                    "data": { "message": e.to_string() },
                })
                .to_string(),
            );
        }
    });

    Ok(Json(
        json!({ "ok": true, "message": "Rule execution started" }),
    ))
}

pub async fn execute_all_rules(
    State(state): State<AppStateRef>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();
    let tx = state.ws_broadcast.clone();
    tokio::task::spawn_blocking(move || {
        let sink = WsRuleSink {
            rule_id: None,
            tx: tx.clone(),
        };
        if let Err(e) = pebble_rules::run_all_rules(store.as_ref(), Some(&sink)) {
            let _ = tx.send(
                json!({
                    "type": "rules_exec_error",
                    "rule_id": null,
                    "data": { "message": e.to_string() },
                })
                .to_string(),
            );
        }
    });

    Ok(Json(
        json!({ "ok": true, "message": "Rule execution started" }),
    ))
}
