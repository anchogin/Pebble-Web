use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{
    extract::{Query, State},
    Json,
};
use pebble_core::KnownContact;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContactsQuery {
    pub account_id: String,
    pub query: Option<String>,
    pub limit: Option<i64>,
}

pub async fn search_contacts(
    State(state): State<AppStateRef>,
    Query(query): Query<SearchContactsQuery>,
) -> Result<Json<Vec<KnownContact>>, ApiError> {
    let contacts = state
        .store
        .list_known_contacts(
            &query.account_id,
            query.query.as_deref().unwrap_or_default(),
            query.limit.unwrap_or(10).clamp(1, 50),
        )
        .map_err(|e| ApiError::Internal(format!("Failed to search contacts: {e}")))?;

    Ok(Json(contacts))
}
