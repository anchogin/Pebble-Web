use crate::magicpush::{encrypt_magicpush_token, MagicPushNotifier};
use axum::{body::Bytes, extract::State, http::HeaderMap, routing::post, Json, Router};
use pebble_core::{AppSettingsRecord, EmailAddress, MagicPushConfigRecord, Message};
use pebble_crypto::CryptoService;
use pebble_mail::StoredMessage;
use pebble_store::Store;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[tokio::test]
async fn send_posts_bearer_markdown_payload_to_magicpush() {
    let captured = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/push", post(capture_request))
        .with_state(captured.clone());
    let notifier = test_notifier_with_config(app).await;

    let sent = notifier
        .send_stored_message(&StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        })
        .await
        .unwrap();

    assert!(sent);
    let request = captured.lock().unwrap().clone().unwrap();
    assert_eq!(request.authorization, "Bearer push-token");
    assert!(request.body.contains("\"title\":\"Deploy report\""));
    assert!(request.body.contains("\"type\":\"markdown\""));
    assert!(request
        .body
        .contains("\"url\":\"https://general.example.com/?messageId=msg%201\""));
}

#[tokio::test]
async fn stored_message_notify_false_does_not_call_magicpush() {
    let captured = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/push", post(capture_request))
        .with_state(captured.clone());
    let notifier = test_notifier_with_config(app).await;

    let sent = notifier
        .send_stored_message(&StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: false,
        })
        .await
        .unwrap();

    assert!(!sent);
    assert!(captured.lock().unwrap().is_none());
}

#[tokio::test]
async fn missing_or_disabled_config_skips_without_calling_magicpush() {
    let captured = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/push", post(capture_request))
        .with_state(captured.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (store, crypto) = test_store_and_crypto();
    let notifier = MagicPushNotifier::new(store, crypto).unwrap();

    let sent = notifier
        .send_stored_message(&StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        })
        .await
        .unwrap();

    assert!(!sent);
    assert!(captured.lock().unwrap().is_none());
}

#[tokio::test]
async fn legacy_magicpush_public_url_is_ignored_when_general_public_url_is_empty() {
    let captured = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/push", post(capture_request))
        .with_state(captured.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (store, crypto) = test_store_and_crypto();
    save_legacy_only_config(&store, &crypto, &format!("http://{}", addr), "push-token");
    let notifier = MagicPushNotifier::new(store, crypto).unwrap();

    let sent = notifier
        .send_stored_message(&StoredMessage {
            message: test_message(),
            folder_ids: vec!["inbox".to_string()],
            notify: true,
        })
        .await
        .unwrap();

    assert!(
        !sent,
        "legacy MagicPush public_url must not be used as fallback"
    );
    assert!(captured.lock().unwrap().is_none());
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

async fn test_notifier_with_config(app: Router) -> MagicPushNotifier {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (store, crypto) = test_store_and_crypto();
    save_test_config(&store, &crypto, &format!("http://{}", addr), "push-token");
    MagicPushNotifier::new(store, crypto).unwrap()
}

fn test_store_and_crypto() -> (Arc<Store>, Arc<CryptoService>) {
    let temp_dir = TempDir::new().unwrap();
    let crypto = Arc::new(CryptoService::init(Some(&temp_dir.path().join("key"))).unwrap());
    let store = Arc::new(Store::open_in_memory().unwrap());
    (store, crypto)
}

fn save_test_config(store: &Store, crypto: &CryptoService, base_url: &str, token: &str) {
    let encrypted = encrypt_magicpush_token(crypto, token).unwrap();
    store
        .save_app_settings(&AppSettingsRecord {
            id: "active".to_string(),
            public_url: "https://general.example.com".to_string(),
            created_at: 1700000000,
            updated_at: 1700000000,
        })
        .unwrap();
    store
        .save_magicpush_config(&MagicPushConfigRecord {
            id: "active".to_string(),
            base_url: base_url.to_string(),
            token_encrypted: Some(encrypted),
            public_url: "https://legacy.example.com".to_string(),
            is_enabled: true,
            created_at: 1700000000,
            updated_at: 1700000000,
        })
        .unwrap();
}

fn save_legacy_only_config(store: &Store, crypto: &CryptoService, base_url: &str, token: &str) {
    let encrypted = encrypt_magicpush_token(crypto, token).unwrap();
    store
        .save_magicpush_config(&MagicPushConfigRecord {
            id: "active".to_string(),
            base_url: base_url.to_string(),
            token_encrypted: Some(encrypted),
            public_url: "https://legacy.example.com".to_string(),
            is_enabled: true,
            created_at: 1700000000,
            updated_at: 1700000000,
        })
        .unwrap();
}

fn test_message() -> Message {
    Message {
        id: "msg 1".to_string(),
        account_id: "account-1".to_string(),
        remote_id: "remote-1".to_string(),
        message_id_header: None,
        in_reply_to: None,
        references_header: None,
        thread_id: None,
        subject: "Deploy report".to_string(),
        snippet: "Build finished".to_string(),
        from_address: "alice@example.com".to_string(),
        from_name: "Alice".to_string(),
        to_list: vec![EmailAddress {
            name: None,
            address: "bob@example.com".to_string(),
        }],
        cc_list: Vec::new(),
        bcc_list: Vec::new(),
        body_text: "Build finished with status ok".to_string(),
        body_html_raw: "".to_string(),
        has_attachments: false,
        is_read: false,
        is_starred: false,
        is_draft: false,
        date: 1700000000,
        remote_version: None,
        is_deleted: false,
        deleted_at: None,
        created_at: 1700000000,
        updated_at: 1700000000,
    }
}
