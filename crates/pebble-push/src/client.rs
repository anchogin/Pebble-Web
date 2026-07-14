use crate::payload::MagicPushPayload;
use crate::types::{PushMessage, ResolvedMagicPushConfig};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct MagicPushClient {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MagicPushError {
    #[error("Failed to build MagicPush HTTP client: {0}")]
    ClientBuild(reqwest::Error),
    #[error("MagicPush request failed: {0}")]
    Request(reqwest::Error),
    #[error("MagicPush returned HTTP {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("MagicPush returned invalid JSON: {0}")]
    InvalidJson(reqwest::Error),
    #[error("{0}")]
    Delivery(String),
}

impl MagicPushClient {
    pub fn new() -> Result<Self, MagicPushError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(MagicPushError::ClientBuild)?;
        Ok(Self { client })
    }

    pub async fn send_message(
        &self,
        message: &PushMessage,
        config: &ResolvedMagicPushConfig,
    ) -> Result<(), MagicPushError> {
        let payload = MagicPushPayload::from_message(message, config);
        self.send_payload(&config.base_url, &config.token, &payload)
            .await
    }

    pub async fn send_test_push(
        &self,
        config: &ResolvedMagicPushConfig,
    ) -> Result<(), MagicPushError> {
        let payload = MagicPushPayload::test(config);
        self.send_payload(&config.base_url, &config.token, &payload)
            .await
    }

    pub async fn send_payload(
        &self,
        base_url: &str,
        token: &str,
        payload: &MagicPushPayload,
    ) -> Result<(), MagicPushError> {
        let response = self
            .client
            .post(format!("{base_url}/api/push"))
            .bearer_auth(token)
            .json(payload)
            .send()
            .await
            .map_err(MagicPushError::Request)?;

        let status = response.status();
        if !status.is_success() {
            if let Ok(result) = response.json::<MagicPushResponse>().await {
                return Err(result.into_http_error(status));
            }
            return Err(MagicPushError::HttpStatus(status));
        }

        let result = response
            .json::<MagicPushResponse>()
            .await
            .map_err(MagicPushError::InvalidJson)?;
        result.into_result()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MagicPushResponse {
    success: Option<bool>,
    success_count: Option<u32>,
    message: Option<String>,
}

impl MagicPushResponse {
    fn into_http_error(self, status: reqwest::StatusCode) -> MagicPushError {
        if let Some(message) = self
            .message
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return MagicPushError::Delivery(message.to_string());
        }
        match self.into_result() {
            Ok(()) => MagicPushError::HttpStatus(status),
            Err(error) => error,
        }
    }

    fn into_result(self) -> Result<(), MagicPushError> {
        if self.success == Some(false) {
            return Err(MagicPushError::Delivery(self.message.unwrap_or_else(
                || "MagicPush reported delivery failure".to_string(),
            )));
        }
        if self.success_count == Some(0) {
            return Err(MagicPushError::Delivery(self.message.unwrap_or_else(
                || "MagicPush delivered to zero channels".to_string(),
            )));
        }
        Ok(())
    }
}
