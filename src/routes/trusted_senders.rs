use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Path, State},
    Json,
};
use pebble_core::{now_timestamp, TrustType, TrustedSender};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustSenderRequest {
    pub email: String,
    pub trust_type: String,
}

fn parse_trust_type(value: &str) -> Result<TrustType, ApiError> {
    match value {
        "images" => Ok(TrustType::Images),
        "all" => Ok(TrustType::All),
        _ => Err(ApiError::BadRequest("Invalid trust type".to_string())),
    }
}

pub async fn list_trusted_senders(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<TrustedSender>>, ApiError> {
    let senders = state
        .store
        .list_trusted_senders(&account_id)
        .map_err(|e| ApiError::Internal(format!("Failed to list trusted senders: {e}")))?;

    Ok(Json(senders))
}

pub async fn trust_sender(
    State(state): State<AppStateRef>,
    Path(account_id): Path<String>,
    Json(body): Json<TrustSenderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = body.email.trim().to_ascii_lowercase();
    if email.is_empty() {
        return Err(ApiError::BadRequest("Email is required".to_string()));
    }

    state
        .store
        .trust_sender(&TrustedSender {
            account_id,
            email,
            trust_type: parse_trust_type(&body.trust_type)?,
            created_at: now_timestamp(),
        })
        .map_err(|e| ApiError::Internal(format!("Failed to trust sender: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_trusted_sender(
    State(state): State<AppStateRef>,
    Path((account_id, email)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = email.trim().to_ascii_lowercase();
    state
        .store
        .remove_trusted_sender(&account_id, &email)
        .map_err(|e| ApiError::Internal(format!("Failed to remove trusted sender: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
