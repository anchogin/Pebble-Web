use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMailOpsQuery {
    pub account_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PendingMailOpsSummaryResponse {
    pub pending_count: i64,
    pub in_progress_count: i64,
    pub failed_count: i64,
    pub total_active_count: i64,
    pub last_error: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PendingMailOpResponse {
    pub id: String,
    pub account_id: String,
    pub message_id: String,
    pub op_type: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_retry_at: Option<i64>,
}

pub async fn pending_ops_summary(
    State(state): State<AppStateRef>,
    Query(query): Query<PendingMailOpsQuery>,
) -> Result<Json<PendingMailOpsSummaryResponse>, ApiError> {
    let account_id = query.account_id;
    let summary = state
        .store
        .pending_mail_ops_summary(account_id.as_deref())
        .map_err(|e| ApiError::Internal(format!("Failed to summarize pending ops: {e}")))?;

    Ok(Json(PendingMailOpsSummaryResponse {
        pending_count: summary.pending_count,
        in_progress_count: summary.in_progress_count,
        failed_count: summary.failed_count,
        total_active_count: summary.total_active_count,
        last_error: summary.last_error,
        updated_at: summary.updated_at,
    }))
}

pub async fn list_pending_ops(
    State(state): State<AppStateRef>,
    Query(query): Query<PendingMailOpsQuery>,
) -> Result<Json<Vec<PendingMailOpResponse>>, ApiError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let ops = state
        .store
        .list_active_pending_mail_ops(query.account_id.as_deref(), limit)
        .map_err(|e| ApiError::Internal(format!("Failed to list pending ops: {e}")))?;

    Ok(Json(
        ops.into_iter()
            .map(|op| PendingMailOpResponse {
                id: op.id,
                account_id: op.account_id,
                message_id: op.message_id,
                op_type: op.op_type,
                status: op.status.as_str().to_string(),
                attempts: op.attempts,
                last_error: op.last_error,
                created_at: op.created_at,
                updated_at: op.updated_at,
                next_retry_at: op.next_retry_at,
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::PendingMailOpsQuery;

    #[test]
    fn pending_ops_query_accepts_missing_account_and_limit() {
        let query: PendingMailOpsQuery = serde_json::from_value(serde_json::json!({})).unwrap();

        assert_eq!(query.account_id, None);
        assert_eq!(query.limit.unwrap_or(100), 100);
    }
}
