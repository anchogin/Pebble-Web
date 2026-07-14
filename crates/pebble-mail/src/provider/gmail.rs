use std::sync::RwLock;

use crate::sync::SyncLogEntry;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use mailparse;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::http_client_with_proxy;
use crate::parser::{AttachmentData, AttachmentMeta};
use pebble_core::traits::{
    AuthCredentials, ChangeSet, DraftProvider, FetchQuery, FetchResult, FolderProvider,
    MailProvider, MailTransport, OutgoingMessage, SyncCursor,
};
use pebble_core::{
    new_id, now_timestamp, DraftMessage, EmailAddress, Folder, FolderRole, FolderType,
    HttpProxyConfig, Message, PebbleError, ProviderCapabilities, Result,
};

const GMAIL_API_BASE: &str = "https://www.googleapis.com/gmail/v1/users/me";

// ---------------------------------------------------------------------------
// Gmail API response types (internal)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailMessageList {
    messages: Option<Vec<GmailMessageRef>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
pub struct GmailMessageRef {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailMessage {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: Option<String>,
    #[serde(rename = "labelIds")]
    label_ids: Option<Vec<String>>,
    snippet: Option<String>,
    payload: Option<GmailPayload>,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailPayload {
    headers: Option<Vec<GmailHeader>>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    body: Option<GmailBody>,
    parts: Option<Vec<GmailPayload>>,
    filename: Option<String>,
}

#[derive(Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailBody {
    size: Option<u64>,
    data: Option<String>,
    #[serde(rename = "attachmentId")]
    attachment_id: Option<String>,
}

#[derive(Deserialize)]
struct GmailLabel {
    id: String,
    name: String,
    #[serde(rename = "type")]
    label_type: Option<String>,
}

#[derive(Deserialize)]
struct GmailLabelList {
    labels: Option<Vec<GmailLabel>>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailHistoryList {
    history: Option<Vec<GmailHistoryEntry>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "historyId")]
    history_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailHistoryEntry {
    #[serde(rename = "messagesAdded")]
    messages_added: Option<Vec<GmailHistoryMessage>>,
    #[serde(rename = "messagesDeleted")]
    messages_deleted: Option<Vec<GmailHistoryMessage>>,
    #[serde(rename = "labelsAdded")]
    labels_added: Option<Vec<GmailHistoryLabelChange>>,
    #[serde(rename = "labelsRemoved")]
    labels_removed: Option<Vec<GmailHistoryLabelChange>>,
}

#[derive(Deserialize)]
struct GmailHistoryMessage {
    message: GmailMessageRef,
}

#[derive(Deserialize)]
struct GmailHistoryLabelChange {
    message: GmailMessageRef,
    #[serde(rename = "labelIds")]
    label_ids: Vec<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailDraft {
    id: String,
    message: Option<GmailMessageRef>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GmailDraftList {
    drafts: Option<Vec<GmailDraft>>,
}

#[derive(Debug, Clone)]
pub struct GmailFetchedMessage {
    pub message: Message,
    pub visible_label_ids: Vec<String>,
    pub attachments: Vec<AttachmentData>,
}

pub fn visible_label_ids(label_ids: &[impl AsRef<str>]) -> Vec<String> {
    label_ids
        .iter()
        .filter_map(|id| {
            let id = id.as_ref();
            if id.starts_with("CATEGORY_")
                || matches!(id, "IMPORTANT" | "STARRED" | "UNREAD" | "CHAT")
            {
                None
            } else {
                Some(id.to_string())
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct GmailAttachmentDescriptor {
    filename: String,
    mime_type: String,
    size: usize,
    content_id: Option<String>,
    is_inline: bool,
    data: Option<Vec<u8>>,
    attachment_id: Option<String>,
}

// ---------------------------------------------------------------------------
// GmailProvider
// ---------------------------------------------------------------------------

pub struct GmailProvider {
    client: Client,
    access_token: RwLock<String>,
    log_tx: Option<mpsc::UnboundedSender<SyncLogEntry>>,
}

impl GmailProvider {
    fn emit_log(&self, action: &str, request: &str) {
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(SyncLogEntry {
                timestamp: Utc::now().timestamp() as u64,
                level: "info".to_string(),
                server: "gmail.googleapis.com".to_string(),
                action: action.to_string(),
                request: Some(request.to_string()),
                ..Default::default()
            });
        }
    }

    pub fn with_log_tx(mut self, tx: mpsc::UnboundedSender<SyncLogEntry>) -> Self {
        self.log_tx = Some(tx);
        self
    }

    pub fn new(access_token: String) -> Self {
        Self {
            client: Client::new(),
            access_token: RwLock::new(access_token),
            log_tx: None,
        }
    }

    pub fn new_with_proxy(access_token: String, proxy: Option<HttpProxyConfig>) -> Result<Self> {
        Ok(Self {
            client: http_client_with_proxy(proxy.as_ref())?,
            access_token: RwLock::new(access_token),
            log_tx: None,
        })
    }

    pub fn set_access_token(&self, token: String) {
        *self.access_token.write().unwrap_or_else(|e| e.into_inner()) = token;
    }

    pub fn token(&self) -> String {
        self.access_token
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(crate) async fn get(&self, url: &str) -> Result<reqwest::Response> {
        self.emit_log("GET", url);
        self.client
            .get(url)
            .bearer_auth(self.token())
            .send()
            .await
            .map_err(|e| PebbleError::Network(format!("Gmail API request failed: {e}")))
    }

    async fn post_json<T: Serialize + Send + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        self.emit_log("POST", url);
        self.client
            .post(url)
            .bearer_auth(self.token())
            .json(body)
            .send()
            .await
            .map_err(|e| PebbleError::Network(format!("Gmail API POST failed: {e}")))
    }

    async fn delete(&self, url: &str) -> Result<reqwest::Response> {
        self.emit_log("DELETE", url);
        self.client
            .delete(url)
            .bearer_auth(self.token())
            .send()
            .await
            .map_err(|e| PebbleError::Network(format!("Gmail API DELETE failed: {e}")))
    }

    fn get_header<'a>(headers: &'a [GmailHeader], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
    }

    async fn fetch_full_gmail_message(&self, gmail_id: &str) -> Result<GmailMessage> {
        let url = format!("{GMAIL_API_BASE}/messages/{gmail_id}?format=full");
        let resp = self.get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "Failed to fetch message {gmail_id} (status {status}): {text}"
            )));
        }
        resp.json()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to parse message {gmail_id}: {e}")))
    }

    async fn fetch_attachment_bytes(&self, gmail_id: &str, attachment_id: &str) -> Result<Vec<u8>> {
        let url = format!("{GMAIL_API_BASE}/messages/{gmail_id}/attachments/{attachment_id}");
        let resp = self.get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "Failed to fetch attachment {attachment_id} for message {gmail_id} (status {status}): {text}"
            )));
        }

        #[derive(Deserialize)]
        struct GmailAttachmentResponse {
            data: Option<String>,
        }

        let attachment: GmailAttachmentResponse = resp
            .json()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to parse attachment body: {e}")))?;

        Ok(attachment
            .data
            .as_deref()
            .map(base64url_decode)
            .unwrap_or_default())
    }

    pub async fn get_profile(&self) -> Result<(String, String)> {
        let url = format!("{GMAIL_API_BASE}/profile");
        let resp = self.get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "Failed to get profile (status {status}): {text}"
            )));
        }

        #[derive(Deserialize)]
        struct ProfileResponse {
            #[serde(rename = "emailAddress")]
            email_address: String,
            #[serde(rename = "historyId")]
            history_id: String,
        }

        let profile: ProfileResponse = resp
            .json()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to parse profile response: {e}")))?;

        Ok((profile.email_address, profile.history_id))
    }

    pub async fn list_message_ids(
        &self,
        label_id: &str,
        limit: u32,
        page_token: Option<&str>,
    ) -> Result<(Vec<GmailMessageRef>, Option<String>)> {
        let mut url = format!("{GMAIL_API_BASE}/messages?labelIds={label_id}&maxResults={limit}");
        if let Some(token) = page_token {
            url.push_str(&format!("&pageToken={token}"));
        }

        let resp = self.get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "Failed to list messages for label {label_id} (status {status}): {text}"
            )));
        }

        let list: GmailMessageList = resp
            .json()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to parse message list: {e}")))?;

        Ok((list.messages.unwrap_or_default(), list.next_page_token))
    }

    pub async fn fetch_sync_message(
        &self,
        gmail_id: &str,
        account_id: &str,
    ) -> Result<GmailFetchedMessage> {
        let gmail_message = self.fetch_full_gmail_message(gmail_id).await?;
        let (message, attachments, remote_label_ids) =
            self.parse_gmail_message(gmail_message, account_id).await?;

        let visible_label_ids = visible_label_ids(&remote_label_ids);

        Ok(GmailFetchedMessage {
            message,
            visible_label_ids,
            attachments,
        })
    }

    // This is a complex beast. It has to recursively walk the MIME parts.
    async fn parse_gmail_message(
        &self,
        msg: GmailMessage,
        account_id: &str,
    ) -> Result<(Message, Vec<AttachmentData>, Vec<String>)> {
        let headers = msg.payload.as_ref().and_then(|p| p.headers.as_ref());
        let empty_headers = Vec::new();
        let headers = headers.unwrap_or(&empty_headers);

        let to_list = Self::get_header(headers, "To")
            .map(parse_address_list)
            .unwrap_or_default();
        let from = Self::get_header(headers, "From")
            .and_then(|val| parse_address(val))
            .unwrap_or_default();

        let date_str = Self::get_header(headers, "Date").unwrap_or_default();
        let date = chrono::DateTime::parse_from_rfc2822(date_str)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        let mut body_text = "".to_string();
        let mut body_html = "".to_string();
        let mut attachments = Vec::new();

        if let Some(payload) = &msg.payload {
            self.walk_payload(
                &msg.id,
                payload,
                &mut body_text,
                &mut body_html,
                &mut attachments,
            )
            .await?;
        }

        let remote_label_ids = msg.label_ids.clone().unwrap_or_default();

        let message = Message {
            id: new_id(),
            account_id: account_id.to_string(),
            remote_id: msg.id.clone(),
            message_id_header: Self::get_header(headers, "Message-ID").map(|s| s.to_string()),
            in_reply_to: Self::get_header(headers, "In-Reply-To").map(|s| s.to_string()),
            references_header: Self::get_header(headers, "References").map(|s| s.to_string()),
            thread_id: msg.thread_id.clone(),
            subject: Self::get_header(headers, "Subject")
                .unwrap_or("")
                .to_string(),
            snippet: msg.snippet.clone().unwrap_or_default(),
            from_address: from.1,
            from_name: from.0.unwrap_or_default(),
            to_list,
            cc_list: Self::get_header(headers, "Cc")
                .map(parse_address_list)
                .unwrap_or_default(),
            bcc_list: Self::get_header(headers, "Bcc")
                .map(parse_address_list)
                .unwrap_or_default(),
            body_text,
            body_html_raw: body_html,
            has_attachments: !attachments.is_empty(),
            is_read: !remote_label_ids.contains(&"UNREAD".to_string()),
            is_starred: remote_label_ids.contains(&"STARRED".to_string()),
            is_draft: remote_label_ids.contains(&"DRAFT".to_string()),
            date,
            remote_version: None,
            is_deleted: false,
            deleted_at: None,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };

        Ok((message, attachments, remote_label_ids))
    }

    async fn walk_payload(
        &self,
        gmail_id: &str,
        payload: &GmailPayload,
        body_text: &mut String,
        body_html: &mut String,
        attachments: &mut Vec<AttachmentData>,
    ) -> Result<()> {
        let mime_type = payload.mime_type.as_deref().unwrap_or("text/plain");
        let has_parts = payload.parts.as_ref().map(|p| p.len()).unwrap_or(0);
        let body_size = payload.body.as_ref().and_then(|b| b.size).unwrap_or(0);
        let has_data = payload
            .body
            .as_ref()
            .and_then(|b| b.data.as_deref())
            .map(|d| d.len())
            .unwrap_or(0);
        tracing::info!(
            "[walk] id={} mime={} parts={} body_size={} has_data={}",
            gmail_id,
            mime_type,
            has_parts,
            body_size,
            has_data
        );

        if let Some(parts) = &payload.parts {
            if !parts.is_empty() {
                for part in parts {
                    Box::pin(self.walk_payload(gmail_id, part, body_text, body_html, attachments))
                        .await?;
                }
                return Ok(());
            }
        }
        if let Some(body) = &payload.body {
            let size = body.size.unwrap_or(0);
            if size > 0 {
                let decoded: Vec<u8> = if let Some(data) = &body.data {
                    base64url_decode(data)
                } else if let Some(attachment_id) = &body.attachment_id {
                    self.fetch_attachment_bytes(gmail_id, attachment_id).await?
                } else {
                    Vec::new()
                };

                if !decoded.is_empty() {
                    match mime_type {
                        "text/plain" if body_text.is_empty() => {
                            *body_text = String::from_utf8(decoded).unwrap_or_default();
                        }
                        "text/html" if body_html.is_empty() => match String::from_utf8(decoded) {
                            Ok(s) => {
                                *body_html = s;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Gmail message {} text/html UTF-8 decode failed: {}",
                                    gmail_id,
                                    e
                                );
                                *body_html = String::from_utf8_lossy(e.as_bytes()).into_owned();
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        let is_attachment = payload
            .headers
            .as_ref()
            .and_then(|h| Self::get_header(h, "Content-Disposition"))
            .map(|v| v.starts_with("attachment"))
            .unwrap_or(false);

        if is_attachment {
            if let (Some(attachment_id), Some(filename), Some(body_size)) = (
                payload
                    .body
                    .as_ref()
                    .and_then(|b| b.attachment_id.as_deref()),
                payload.filename.as_deref(),
                payload.body.as_ref().and_then(|b| b.size),
            ) {
                if !filename.is_empty() && body_size > 0 {
                    let data = self.fetch_attachment_bytes(gmail_id, attachment_id).await?;
                    attachments.push(AttachmentData {
                        meta: AttachmentMeta {
                            filename: filename.to_string(),
                            mime_type: mime_type.to_string(),
                            size: data.len(),
                            content_id: payload
                                .headers
                                .as_ref()
                                .and_then(|h| Self::get_header(h, "Content-ID"))
                                .map(|s| s.to_string()),
                            is_inline: false, // Heuristic
                        },
                        data,
                    });
                }
            }
        }
        Ok(())
    }
}

fn parse_address_list(value: &str) -> Vec<EmailAddress> {
    let Ok(list) = mailparse::addrparse(value) else {
        return vec![];
    };
    let mut emails = vec![];
    for addr in list.iter() {
        match addr {
            mailparse::MailAddr::Single(single) => {
                emails.push(EmailAddress {
                    name: single.display_name.clone(),
                    address: single.addr.clone(),
                });
            }
            mailparse::MailAddr::Group(group) => {
                for single in &group.addrs {
                    emails.push(EmailAddress {
                        name: single.display_name.clone(),
                        address: single.addr.clone(),
                    });
                }
            }
        }
    }
    emails
}

fn parse_address(value: &str) -> Option<(Option<String>, String)> {
    let Ok(list) = mailparse::addrparse(value) else {
        return None;
    };
    for addr in list.iter() {
        match addr {
            mailparse::MailAddr::Single(single) => {
                return Some((single.display_name.clone(), single.addr.clone()));
            }
            mailparse::MailAddr::Group(group) => {
                if let Some(single) = group.addrs.first() {
                    return Some((single.display_name.clone(), single.addr.clone()));
                }
            }
        }
    }
    None
}

fn base64url_decode(input: &str) -> Vec<u8> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| general_purpose::URL_SAFE.decode(input))
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(input))
        .or_else(|_| general_purpose::STANDARD.decode(input))
        .unwrap_or_default()
}

#[async_trait]
impl FolderProvider for GmailProvider {
    async fn list_folders(&self) -> Result<Vec<Folder>> {
        let url = format!("{GMAIL_API_BASE}/labels");
        let resp = self.get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PebbleError::Network(format!(
                "Failed to list labels (status {status}): {text}"
            )));
        }

        let label_list: GmailLabelList = resp
            .json()
            .await
            .map_err(|e| PebbleError::Network(format!("Failed to parse labels: {e}")))?;

        let mut folders = Vec::new();
        if let Some(labels) = label_list.labels {
            for label in labels {
                let role = match label.id.as_str() {
                    "INBOX" => Some(FolderRole::Inbox),
                    "SENT" => Some(FolderRole::Sent),
                    "DRAFT" => Some(FolderRole::Drafts),
                    "TRASH" => Some(FolderRole::Trash),
                    "SPAM" => Some(FolderRole::Spam),
                    _ => None,
                };

                let folder = Folder {
                    id: new_id(),
                    account_id: "".to_string(), // Filled in by caller
                    remote_id: label.id,
                    name: label.name,
                    folder_type: FolderType::Label,
                    role,
                    parent_id: None,
                    color: None,
                    is_system: label.label_type == Some("system".to_string()),
                    server_linked: true,
                    sort_order: 0, // Filled in by caller
                };
                folders.push(folder);
            }
        }
        Ok(folders)
    }

    async fn move_message(&self, _message_id: &str, _folder_id: &str) -> Result<String> {
        Err(PebbleError::Internal(
            "Gmail does not support moving messages between folders. Use labels instead."
                .to_string(),
        ))
    }
}

#[async_trait]
impl MailTransport for GmailProvider {
    async fn authenticate(&mut self, _credentials: &AuthCredentials) -> Result<()> {
        // For Gmail, auth is handled via token, so this is a no-op
        Ok(())
    }

    async fn fetch_messages(&self, _query: &FetchQuery) -> Result<FetchResult> {
        Err(PebbleError::Internal(
            "fetch_messages is not supported for Gmail. Use the sync worker instead.".to_string(),
        ))
    }

    async fn send_message(&self, _message: &OutgoingMessage) -> Result<()> {
        Err(PebbleError::Internal(
            "send_message is not yet implemented for Gmail".to_string(),
        ))
    }

    async fn sync_changes(&self, _since: &SyncCursor) -> Result<ChangeSet> {
        Err(PebbleError::Internal(
            "sync_changes is not supported for Gmail. Use the sync worker instead.".to_string(),
        ))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            has_labels: true,
            has_folders: false,
            has_categories: false,
            has_push: false,
            has_threads: true,
        }
    }
}

impl MailProvider for GmailProvider {}
