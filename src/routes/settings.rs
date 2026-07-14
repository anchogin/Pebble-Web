use crate::error::ApiError;
use crate::state::AppStateRef;
use axum::{extract::State, Json};
use pebble_core::{now_timestamp, AppSettingsRecord};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettingsResponse {
    pub public_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGeneralSettingsRequest {
    pub public_url: String,
}

pub async fn get_general_settings(
    State(state): State<AppStateRef>,
) -> Result<Json<GeneralSettingsResponse>, ApiError> {
    let settings = state
        .store
        .get_app_settings()
        .map_err(|e| ApiError::Internal(format!("Failed to load settings: {e}")))?;

    Ok(Json(GeneralSettingsResponse {
        public_url: settings.public_url,
    }))
}

pub async fn save_general_settings(
    State(state): State<AppStateRef>,
    Json(body): Json<SaveGeneralSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let existing = state
        .store
        .get_app_settings()
        .map_err(|e| ApiError::Internal(format!("Failed to load settings: {e}")))?;
    let now = now_timestamp();
    state
        .store
        .save_app_settings(&AppSettingsRecord {
            id: "active".to_string(),
            public_url: body.public_url,
            created_at: existing.created_at,
            updated_at: now,
        })
        .map_err(|e| ApiError::Internal(format!("Failed to save settings: {e}")))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, Json};

    #[tokio::test]
    async fn general_settings_save_and_load_public_url() {
        let state = crate::routes::magicpush::tests::test_state_ref();

        let _ = super::save_general_settings(
            State(state.clone()),
            Json(super::SaveGeneralSettingsRequest {
                public_url: "https://mail.example.com/".to_string(),
            }),
        )
        .await
        .unwrap();

        let Json(settings) = super::get_general_settings(State(state)).await.unwrap();

        assert_eq!(settings.public_url, "https://mail.example.com");
    }
}
