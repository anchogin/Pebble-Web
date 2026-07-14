use axum::{body::Bytes, extract::State, http::HeaderMap, routing::post, Json, Router};
use pebble_push::{
    decrypt_magicpush_token, encrypt_magicpush_token, resolve_magicpush_config, MagicPushClient,
    MagicPushPayload, PushEmailAddress, PushMessage, ResolvedMagicPushConfig,
    StoredMagicPushConfig,
};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[test]
fn payload_contains_required_mail_fields_and_link() {
    let message = test_message();
    let config = test_config("https://push.example.com");

    let payload = MagicPushPayload::from_message(&message, &config);

    assert_eq!(payload.title, "Deploy report");
    assert_eq!(payload.r#type, "markdown");
    assert_eq!(payload.url, "https://mail.example.com/?messageId=msg%201");
    assert!(payload
        .content
        .contains("**发件人:** Alice <alice@example.com>"));
    assert!(payload.content.contains("**标题:** Deploy report"));
    assert!(payload.content.contains("**收件人:** bob@example.com"));
    assert!(payload
        .content
        .contains("**正文:** Build finished with status ok"));
    assert!(payload
        .content
        .contains("**邮件时间:** 2023-11-14 22:13:20 UTC"));
    assert!(payload
        .content
        .contains("**邮件链接:** https://mail.example.com/?messageId=msg%201"));
}

#[test]
fn config_resolution_skips_disabled_or_incomplete_records() {
    let mut record = StoredMagicPushConfig {
        base_url: "https://push.example.com/".to_string(),
        token_encrypted: Some("encrypted".to_string()),
        public_url: "https://mail.example.com/".to_string(),
        is_enabled: true,
    };

    let resolved = resolve_magicpush_config(&record, "push-token".to_string()).unwrap();
    assert_eq!(resolved.base_url, "https://push.example.com");
    assert_eq!(resolved.public_url, "https://mail.example.com");

    record.is_enabled = false;
    assert!(resolve_magicpush_config(&record, "push-token".to_string()).is_none());
}

#[test]
fn token_codec_uses_hex_around_crypto_closures() {
    let encrypted = encrypt_magicpush_token("push-token", |plaintext| {
        let mut bytes = plaintext.to_vec();
        bytes.reverse();
        Ok::<_, String>(bytes)
    })
    .unwrap();

    assert_eq!(encrypted, "6e656b6f742d68737570");
    let decrypted = decrypt_magicpush_token(&encrypted, |ciphertext| {
        let mut bytes = ciphertext.to_vec();
        bytes.reverse();
        Ok::<_, String>(bytes)
    })
    .unwrap();
    assert_eq!(decrypted, "push-token");
}

#[tokio::test]
async fn client_posts_bearer_markdown_payload_to_magicpush() {
    let captured = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/push", post(capture_request))
        .with_state(captured.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = MagicPushClient::new().unwrap();

    client
        .send_message(&test_message(), &test_config(&format!("http://{addr}")))
        .await
        .unwrap();

    let request = captured.lock().unwrap().clone().unwrap();
    assert_eq!(request.authorization, "Bearer push-token");
    assert!(request.body.contains("\"title\":\"Deploy report\""));
    assert!(request.body.contains("\"type\":\"markdown\""));
    assert!(request
        .body
        .contains("\"url\":\"https://mail.example.com/?messageId=msg%201\""));
}

#[tokio::test]
async fn client_treats_magicpush_json_failure_as_error() {
    let app = Router::new().route(
        "/api/push",
        post(|| async {
            Json(serde_json::json!({
                "success": false,
                "successCount": 0,
                "message": "no channel delivered"
            }))
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = MagicPushClient::new().unwrap();

    let error = client
        .send_message(&test_message(), &test_config(&format!("http://{addr}")))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("no channel delivered"));
}

#[tokio::test]
async fn client_surfaces_magicpush_400_json_message() {
    let app = Router::new().route(
        "/api/push",
        post(|| async {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "code": 400,
                    "message": "无效的接口令牌"
                })),
            )
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = MagicPushClient::new().unwrap();

    let error = client
        .send_message(&test_message(), &test_config(&format!("http://{addr}")))
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("无效的接口令牌"),
        "expected MagicPush response body message, got: {error}"
    );
}

#[tokio::test]
async fn client_surfaces_magicpush_400_message_without_success_flag() {
    let app = Router::new().route(
        "/api/push",
        post(|| async {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "code": 400,
                    "message": "无效的接口令牌"
                })),
            )
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = MagicPushClient::new().unwrap();

    let error = client
        .send_message(&test_message(), &test_config(&format!("http://{addr}")))
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("无效的接口令牌"),
        "expected MagicPush response body message without success flag, got: {error}"
    );
}

#[derive(Clone, Debug)]
struct CapturedRequest {
    authorization: String,
    body: String,
}

async fn capture_request(
    State(captured): State<Arc<Mutex<Option<CapturedRequest>>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<serde_json::Value> {
    *captured.lock().unwrap() = Some(CapturedRequest {
        authorization: headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        body: String::from_utf8(body.to_vec()).unwrap(),
    });
    Json(serde_json::json!({
        "success": true,
        "successCount": 1,
        "failedCount": 0
    }))
}

fn test_config(base_url: &str) -> ResolvedMagicPushConfig {
    ResolvedMagicPushConfig {
        base_url: base_url.to_string(),
        token: "push-token".to_string(),
        public_url: "https://mail.example.com".to_string(),
    }
}

fn test_message() -> PushMessage {
    PushMessage {
        id: "msg 1".to_string(),
        subject: "Deploy report".to_string(),
        snippet: "Build finished".to_string(),
        from_address: "alice@example.com".to_string(),
        from_name: "Alice".to_string(),
        to_list: vec![PushEmailAddress {
            name: None,
            address: "bob@example.com".to_string(),
        }],
        body_text: "Build finished with status ok".to_string(),
        date: 1700000000,
    }
}
