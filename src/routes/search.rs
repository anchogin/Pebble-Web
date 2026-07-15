use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{extract::State, Json};
use pebble_core::traits::SearchHit;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub has_attachment: Option<bool>,
    pub folder_id: Option<String>,
    pub limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_accepts_camel_case_date_from() {
        let body: SearchRequest = serde_json::from_str(r#"{"dateFrom":1722700800}"#).unwrap();

        assert_eq!(body.date_from, Some(1_722_700_800));
        assert_eq!(body.date_to, None);
    }
}

pub async fn search_messages(
    State(state): State<AppStateRef>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    // Determine if this is a simple search or advanced
    let is_advanced = body.from.is_some()
        || body.to.is_some()
        || body.subject.is_some()
        || body.date_from.is_some()
        || body.date_to.is_some()
        || body.has_attachment.is_some()
        || body.folder_id.is_some();

    let hits = if is_advanced {
        state
            .store
            .advanced_search_messages(
                body.query.as_deref(),
                body.from.as_deref(),
                body.to.as_deref(),
                body.subject.as_deref(),
                body.date_from,
                body.date_to,
                body.has_attachment,
                body.folder_id.as_deref(),
                body.limit,
            )
            .map_err(|e| ApiError::Internal(format!("Search failed: {e}")))?
    } else {
        let query_text = body.query.unwrap_or_default();
        if query_text.is_empty() {
            return Ok(Json(Vec::new()));
        }
        state
            .store
            .search_messages_wide(&query_text, body.limit)
            .map_err(|e| ApiError::Internal(format!("Search failed: {e}")))?
    };

    Ok(Json(hits))
}
