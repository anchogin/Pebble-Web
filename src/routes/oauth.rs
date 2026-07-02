use crate::credentials::{encrypt_credentials, AccountCredentials, GmailCredentials};
use crate::error::ApiError;
use crate::state::{AppStateRef, OAuthSession, OAuthSessionResult};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    Json,
};
use pebble_core::{new_id, now_timestamp, Account, ProviderType};
use pebble_oauth::{OAuthConfig, OAuthManager, OAuthNetworkConfig};
use serde::{Deserialize, Serialize};
use tracing::info;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GOOGLE_SCOPES: &[&str] = &[
    "https://mail.google.com/",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
];

pub fn google_oauth_manager(config: &crate::config::Config) -> Result<OAuthManager, ApiError> {
    let client_id = config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("GOOGLE_CLIENT_ID not configured".into()))?
        .to_string();
    let client_secret = config.google_client_secret.clone();
    Ok(OAuthManager::new_with_network(
        OAuthConfig {
            client_id,
            client_secret,
            auth_url: GOOGLE_AUTH_URL.to_string(),
            token_url: GOOGLE_TOKEN_URL.to_string(),
            scopes: GOOGLE_SCOPES.iter().map(|s| s.to_string()).collect(),
            redirect_port: 0,
        },
        OAuthNetworkConfig::default(),
    ))
}

fn build_redirect_uri(state: &AppStateRef) -> String {
    let base = state
        .config
        .public_url
        .as_deref()
        .unwrap_or("http://localhost:8080");
    format!("{}/api/v1/oauth/google/callback", base.trim_end_matches('/'))
}

fn set_session_error(sessions: &mut std::collections::HashMap<String, OAuthSession>, id: &str, msg: String) {
    sessions.insert(
        id.to_string(),
        OAuthSession {
            pkce_state: None,
            display_name: String::new(),
            result: OAuthSessionResult::Error(msg),
        },
    );
}

#[derive(Serialize)]
pub struct StartOAuthResponse {
    pub session_id: String,
    pub auth_url: String,
}

pub async fn start_google_oauth(
    State(state): State<AppStateRef>,
) -> Result<Json<StartOAuthResponse>, ApiError> {
    let redirect_uri = build_redirect_uri(&state);
    let manager = google_oauth_manager(&state.config)?;
    let session_id = new_id();

    let (auth_url, pkce_state) = manager
        .start_auth_with_redirect_and_state(&redirect_uri, Some(session_id.clone()))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to start OAuth: {e}")))?;

    state.oauth_sessions.lock().await.insert(
        session_id.clone(),
        OAuthSession {
            pkce_state: Some(pkce_state),
            display_name: String::new(),
            result: OAuthSessionResult::Pending,
        },
    );

    info!("Google OAuth session started: {}, redirect_uri: {}, auth_url: {}", session_id, redirect_uri, auth_url);
    Ok(Json(StartOAuthResponse { session_id, auth_url }))
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn google_oauth_callback(
    State(app_state): State<AppStateRef>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    info!("Google OAuth callback received: code={:?}, state={:?}, error={:?}",
        params.code.as_deref().map(|c| &c[..c.len().min(10)]),
        params.state,
        params.error
    );
    let session_id = match params.state {
        Some(s) => s,
        None => return Html(error_page("Missing state parameter")).into_response(),
    };

    if let Some(err) = params.error {
        let mut sessions = app_state.oauth_sessions.lock().await;
        set_session_error(&mut sessions, &session_id, format!("Google denied access: {err}"));
        return Html(error_page(&format!("Google denied access: {err}"))).into_response();
    }

    let code = match params.code {
        Some(c) => c,
        None => {
            let mut sessions = app_state.oauth_sessions.lock().await;
            set_session_error(&mut sessions, &session_id, "No authorization code received".into());
            return Html(error_page("No authorization code received")).into_response();
        }
    };

    let pkce_state = {
        let mut sessions = app_state.oauth_sessions.lock().await;
        match sessions.remove(&session_id) {
            Some(s) => match s.pkce_state {
                Some(p) => p,
                None => return Html(error_page("Session has no PKCE state")).into_response(),
            },
            None => return Html(error_page("Invalid or expired session")).into_response(),
        }
    };

    let redirect_uri = build_redirect_uri(&app_state);
    let manager = match google_oauth_manager(&app_state.config) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("Config error: {e}");
            let mut sessions = app_state.oauth_sessions.lock().await;
            set_session_error(&mut sessions, &session_id, msg.clone());
            return Html(error_page(&msg)).into_response();
        }
    };

    let token_pair = match manager
        .complete_auth_with_redirect(&code, pkce_state, &redirect_uri)
        .await
    {
        Ok(pair) => pair,
        Err(e) => {
            let msg = format!("Token exchange failed: {e}");
            let mut sessions = app_state.oauth_sessions.lock().await;
            set_session_error(&mut sessions, &session_id, msg.clone());
            return Html(error_page(&msg)).into_response();
        }
    };

    let (email, display_name) = match fetch_google_userinfo(&token_pair.access_token).await {
        Ok(info) => (info.email, info.name.unwrap_or_default()),
        Err(e) => {
            let msg = format!("Failed to fetch user info: {e}");
            let mut sessions = app_state.oauth_sessions.lock().await;
            set_session_error(&mut sessions, &session_id, msg.clone());
            return Html(error_page(&msg)).into_response();
        }
    };

    let encrypted = match encrypt_credentials(
        &app_state.crypto,
        &AccountCredentials::Gmail(GmailCredentials {
            access_token: token_pair.access_token.clone(),
            refresh_token: token_pair.refresh_token.clone(),
            expires_at: token_pair.expires_at,
            email: email.clone(),
        }),
    ) {
        Ok(e) => e,
        Err(e) => {
            let msg = format!("Encryption failed: {e}");
            let mut sessions = app_state.oauth_sessions.lock().await;
            set_session_error(&mut sessions, &session_id, msg.clone());
            return Html(error_page(&msg)).into_response();
        }
    };

    let now = now_timestamp();
    let account = Account {
        id: new_id(),
        email: email.clone(),
        display_name: display_name.clone(),
        color: None,
        provider: ProviderType::Gmail,
        created_at: now,
        updated_at: now,
    };

    let sync_state = serde_json::json!({
        "credentials": encrypted,
        "google_client_id": app_state.config.google_client_id,
        "google_client_secret": app_state.config.google_client_secret,
        "sync_strategy": "recent",
    })
    .to_string();

    let account_clone = account.clone();
    if let Err(e) = app_state
        .store
        .with_write_async(move |conn| {
            conn.execute(
                "INSERT INTO accounts \
                 (id, email, display_name, color, provider, created_at, updated_at, sync_state) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    account_clone.id,
                    account_clone.email,
                    account_clone.display_name,
                    account_clone.color.as_deref(),
                    "gmail",
                    account_clone.created_at,
                    account_clone.updated_at,
                    sync_state,
                ],
            )?;
            Ok(())
        })
        .await
    {
        let msg = format!("Failed to save account: {e}");
        let mut sessions = app_state.oauth_sessions.lock().await;
        set_session_error(&mut sessions, &session_id, msg.clone());
        return Html(error_page(&msg)).into_response();
    }

    let account_id = account.id.clone();
    let sync_manager = app_state.sync_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = sync_manager.start_account_sync(&account_id).await {
            tracing::warn!("Failed to auto-start Gmail sync for {account_id}: {e}");
        }
    });

    app_state.oauth_sessions.lock().await.insert(
        session_id.clone(),
        OAuthSession {
            pkce_state: None,
            display_name: display_name.clone(),
            result: OAuthSessionResult::Complete {
                account_id: account.id.clone(),
                email: email.clone(),
            },
        },
    );

    info!("Google OAuth complete for {}", email);
    Html(success_page(&email)).into_response()
}

#[derive(Serialize)]
#[serde(tag = "status")]
pub enum OAuthStatusResponse {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "complete")]
    Complete { account_id: String, email: String },
    #[serde(rename = "error")]
    Error { message: String },
}

pub async fn google_oauth_status(
    State(state): State<AppStateRef>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<OAuthStatusResponse>, ApiError> {
    let sessions = state.oauth_sessions.lock().await;
    match sessions.get(&session_id) {
        None => Err(ApiError::NotFound("Session not found or expired".into())),
        Some(s) => match &s.result {
            OAuthSessionResult::Pending => Ok(Json(OAuthStatusResponse::Pending)),
            OAuthSessionResult::Complete { account_id, email } => {
                Ok(Json(OAuthStatusResponse::Complete {
                    account_id: account_id.clone(),
                    email: email.clone(),
                }))
            }
            OAuthSessionResult::Error(msg) => Ok(Json(OAuthStatusResponse::Error {
                message: msg.clone(),
            })),
        },
    }
}

#[derive(Deserialize)]
pub struct GoogleUserInfo {
    pub email: String,
    pub name: Option<String>,
}

pub async fn fetch_google_userinfo(access_token: &str) -> Result<GoogleUserInfo, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Userinfo request failed: HTTP {}", resp.status()));
    }

    resp.json::<GoogleUserInfo>()
        .await
        .map_err(|e| format!("Failed to parse userinfo: {e}"))
}

fn success_page(email: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><title>Pebble – Sign in successful</title><meta charset="utf-8">
<style>body{{font-family:system-ui,sans-serif;text-align:center;padding:3rem;background:#0f1117;color:#e2e8f0}}
h2{{color:#4ade80}}p{{color:#94a3b8}}button{{margin-top:1.5rem;padding:10px 24px;border-radius:8px;border:none;background:#3b82f6;color:#fff;font-size:14px;cursor:pointer}}</style></head>
<body>
<h2>✓ Signed in successfully</h2>
<p>Signed in as <strong>{}</strong></p>
<p>You can close this window and return to Pebble.</p>
<button onclick="window.close()">Close window</button>
</body></html>"#,
        email
    )
}

fn error_page(message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><title>Pebble – Sign in failed</title><meta charset="utf-8">
<style>body{{font-family:system-ui,sans-serif;text-align:center;padding:3rem;background:#0f1117;color:#e2e8f0}}
h2{{color:#f87171}}p{{color:#94a3b8}}button{{margin-top:1.5rem;padding:10px 24px;border-radius:8px;border:none;background:#3b82f6;color:#fff;font-size:14px;cursor:pointer}}</style></head>
<body>
<h2>Sign in failed</h2>
<p>{}</p>
<button onclick="window.close()">Close window</button>
</body></html>"#,
        message
    )
}
