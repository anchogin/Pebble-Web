use crate::error::ApiError;
use crate::state::{ActiveRuleRun, AppStateRef, RuleRunKey};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use pebble_core::{new_id, now_timestamp, Rule};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
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

#[derive(Deserialize)]
pub struct ExecuteRuleQuery {
    pub run_id: Option<String>,
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
    rule_name: String,
    run_id: String,
    tx: broadcast::Sender<String>,
}

fn cleanup_rule_run(
    runs: &tokio::sync::Mutex<std::collections::HashMap<RuleRunKey, ActiveRuleRun>>,
    key: &RuleRunKey,
    run_id: &str,
) {
    let mut guard = runs.blocking_lock();
    if guard.get(key).is_some_and(|active| active.run_id == run_id) {
        guard.remove(key);
    }
}

impl pebble_rules::ProgressSink for WsRuleSink {
    fn emit(&self, ev: pebble_rules::RuleProgressEvent) {
        let mut value = serde_json::to_value(&ev).unwrap_or_else(|_| json!({}));
        if let Some(obj) = value.as_object_mut() {
            obj.insert("rule_id".to_string(), json!(self.rule_id.as_ref()));
            obj.insert("run_id".to_string(), json!(self.run_id));
        }
        let phase = value
            .get("phase")
            .and_then(|phase| phase.as_str())
            .unwrap_or("error")
            .to_lowercase();
        let wrap = json!({
            "type": format!("rules_exec_{phase}"),
            "rule_id": self.rule_id.as_ref(),
            "run_id": self.run_id,
            "data": value,
        });
        tracing::info!(
            rule_id = ?self.rule_id,
            rule_name = %self.rule_name,
            run_id = %self.run_id,
            phase = %phase,
            payload = %wrap,
            "[执行规则]{}:发送WebSocket事件", self.rule_name
        );
        let _ = self.tx.send(wrap.to_string());
    }
}

pub async fn execute_rule(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
    Query(query): Query<ExecuteRuleQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rule = state
        .store
        .list_all_rules()
        .map_err(|e| ApiError::Internal(format!("Failed to list rules: {e}")))?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| ApiError::NotFound("Rule not found".to_string()))?;

    let rule_id = rule.id.clone();
    let rule_name = rule.name.clone();
    let run_id = query.run_id.unwrap_or_else(new_id);
    let response_run_id = run_id.clone();
    let run_key = RuleRunKey::Single(rule_id.clone());
    let control = Arc::new(pebble_rules::RunControl::new());
    {
        let mut runs = state.rule_runs.lock().await;
        if let Some(active) = runs.insert(
            run_key.clone(),
            ActiveRuleRun {
                run_id: run_id.clone(),
                control: control.clone(),
            },
        ) {
            tracing::info!(
                rule_id = %rule_id,
                rule_name = %rule_name,
                old_run_id = %active.run_id,
                new_run_id = %run_id,
                "[执行规则]{}:取消旧执行", rule_name
            );
            active.control.cancel();
        }
    }
    let store = state.store.clone();
    let tx = state.ws_broadcast.clone();
    let runs = state.rule_runs.clone();
    tracing::info!(rule_id = %rule_id, rule_name = %rule_name, run_id = %run_id, "[执行规则]{}:收到执行请求", rule_name);
    tokio::task::spawn_blocking(move || {
        tracing::info!(rule_id = %rule_id, rule_name = %rule_name, run_id = %run_id, "[执行规则]{}:后台任务开始", rule_name);
        let sink = WsRuleSink {
            rule_id: Some(rule_id.clone()),
            rule_name: rule_name.clone(),
            run_id: run_id.clone(),
            tx: tx.clone(),
        };
        if let Err(e) =
            pebble_rules::run_single_rule(&rule, store.as_ref(), Some(&sink), Some(&control))
        {
            tracing::warn!(rule_id = %rule_id, rule_name = %rule_name, run_id = %run_id, error = %e, "[执行规则]{}:后台任务失败", rule_name);
            let _ = tx.send(
                json!({
                    "type": "rules_exec_error",
                    "rule_id": rule_id,
                    "run_id": run_id.clone(),
                    "data": { "message": e.to_string() },
                })
                .to_string(),
            );
        } else {
            tracing::info!(rule_id = %rule_id, rule_name = %rule_name, run_id = %run_id, "[执行规则]{}:后台任务结束", rule_name);
        }
        cleanup_rule_run(&runs, &run_key, &run_id);
    });

    Ok(Json(
        json!({ "ok": true, "message": "Rule execution started", "run_id": response_run_id }),
    ))
}

pub async fn cancel_rule_run(
    State(state): State<AppStateRef>,
    Path(id): Path<String>,
    Query(query): Query<ExecuteRuleQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = RuleRunKey::Single(id.clone());
    let runs = state.rule_runs.lock().await;
    if let Some(active) = runs.get(&key) {
        if query
            .run_id
            .as_deref()
            .is_none_or(|run_id| run_id == active.run_id)
        {
            tracing::info!(rule_id = %id, run_id = %active.run_id, "rules: cancel_rule_run requested");
            active.control.cancel();
        } else {
            tracing::info!(rule_id = %id, run_id = ?query.run_id, active_run_id = %active.run_id, "rules: cancel_rule_run ignored mismatched run_id");
        }
    } else {
        tracing::info!(rule_id = %id, run_id = ?query.run_id, "rules: cancel_rule_run requested for unknown run");
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn execute_all_rules(
    State(state): State<AppStateRef>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store.clone();
    let tx = state.ws_broadcast.clone();
    let run_id = new_id();
    let run_key = RuleRunKey::All;
    let control = Arc::new(pebble_rules::RunControl::new());
    {
        let mut runs = state.rule_runs.lock().await;
        if let Some(active) = runs.insert(
            run_key.clone(),
            ActiveRuleRun {
                run_id: run_id.clone(),
                control: control.clone(),
            },
        ) {
            tracing::info!(old_run_id = %active.run_id, new_run_id = %run_id, "rules: cancelling prior all-rules run");
            active.control.cancel();
        }
    }
    let runs = state.rule_runs.clone();
    tracing::info!(run_id = %run_id, "rules: execute_all_rules requested");
    tokio::task::spawn_blocking(move || {
        tracing::info!(run_id = %run_id, "rules: execute_all_rules worker started");
        let sink = WsRuleSink {
            rule_id: None,
            rule_name: "全部规则".to_string(),
            run_id: run_id.clone(),
            tx: tx.clone(),
        };
        if let Err(e) = pebble_rules::run_all_rules(store.as_ref(), Some(&sink), Some(&control)) {
            tracing::warn!(run_id = %run_id, error = %e, "rules: execute_all_rules worker failed");
            let _ = tx.send(
                json!({
                    "type": "rules_exec_error",
                    "rule_id": null,
                    "run_id": run_id.clone(),
                    "data": { "message": e.to_string() },
                })
                .to_string(),
            );
        } else {
            tracing::info!(run_id = %run_id, "rules: execute_all_rules worker finished");
        }
        cleanup_rule_run(&runs, &run_key, &run_id);
    });

    Ok(Json(
        json!({ "ok": true, "message": "Rule execution started" }),
    ))
}

pub async fn cancel_all_rules_run(
    State(state): State<AppStateRef>,
    Query(query): Query<ExecuteRuleQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let runs = state.rule_runs.lock().await;
    if let Some(active) = runs.get(&RuleRunKey::All) {
        if query
            .run_id
            .as_deref()
            .is_none_or(|run_id| run_id == active.run_id)
        {
            tracing::info!(run_id = %active.run_id, "rules: cancel_all_rules_run requested");
            active.control.cancel();
        } else {
            tracing::info!(run_id = ?query.run_id, active_run_id = %active.run_id, "rules: cancel_all_rules_run ignored mismatched run_id");
        }
    } else {
        tracing::info!(run_id = ?query.run_id, "rules: cancel_all_rules_run requested for unknown run");
    }

    Ok(Json(json!({ "ok": true })))
}
