pub mod accounts;
pub mod attachments;
pub mod auth;
pub mod compose;
pub mod folders;
pub mod health;
pub mod messages;
pub mod search;
pub mod sync;
pub mod threads;

use crate::state::AppStateRef;
use crate::ws;
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::services::ServeDir;

pub fn build_router(state: AppStateRef, static_dir: &str) -> Router {
    let jwt_secret = state.config.jwt_secret.clone();

    let public_routes = Router::new()
        .route("/api/v1/health", get(health::health))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/ws", get(ws::ws_handler));

    let protected_routes = Router::new()
        // Accounts
        .route("/api/v1/accounts", get(accounts::list_accounts))
        .route("/api/v1/accounts", post(accounts::create_account))
        .route("/api/v1/accounts/{account_id}", delete(accounts::delete_account))
        // Folders
        .route("/api/v1/accounts/{account_id}/folders", get(folders::list_folders))
        // Messages
        .route("/api/v1/folders/{folder_id}/messages", get(messages::list_messages_by_folder))
        .route("/api/v1/messages/{message_id}", get(messages::get_message))
        .route("/api/v1/messages/{message_id}/flags", put(messages::update_message_flags))
        .route("/api/v1/messages/{message_id}/move", post(messages::move_message))
        .route("/api/v1/messages/{message_id}", delete(messages::delete_message))
        // Threads
        .route("/api/v1/folders/{folder_id}/threads", get(threads::list_threads_by_folder))
        .route("/api/v1/threads/{thread_id}/messages", get(threads::get_thread_messages))
        // Search
        .route("/api/v1/search", post(search::search_messages))
        // Attachments
        .route("/api/v1/messages/{message_id}/attachments", get(attachments::list_attachments))
        .route("/api/v1/attachments/{attachment_id}/download", get(attachments::download_attachment))
        // Compose
        .route("/api/v1/compose", post(compose::send_email))
        // Sync
        .route("/api/v1/sync/trigger", post(sync::trigger_sync))
        .layer(middleware::from_fn(move |req, next| {
            crate::auth::auth_middleware(jwt_secret.clone(), req, next)
        }));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true))
        .with_state(state)
}
