use crate::types::{PushEmailAddress, PushMessage, ResolvedMagicPushConfig};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MagicPushPayload {
    pub title: String,
    pub content: String,
    pub r#type: &'static str,
    pub url: String,
}

impl MagicPushPayload {
    pub fn from_message(message: &PushMessage, config: &ResolvedMagicPushConfig) -> Self {
        let message_url = format!(
            "{}/?messageId={}",
            config.public_url,
            percent_encode(&message.id)
        );
        let title = if message.subject.trim().is_empty() {
            "新邮件".to_string()
        } else {
            message.subject.clone()
        };
        let from = format_sender(message);
        let recipients = format_addresses(&message.to_list);
        let body = if message.body_text.trim().is_empty() {
            message.snippet.trim()
        } else {
            message.body_text.trim()
        };
        let content = format!(
            "**发件人:** {from}\n\n**标题:** {title}\n\n**收件人:** {recipients}\n\n**正文:** {}\n\n**邮件时间:** {}\n\n**邮件链接:** {message_url}",
            truncate_body(body),
            format_mail_time(message.date)
        );

        Self {
            title,
            content,
            r#type: "markdown",
            url: message_url,
        }
    }

    pub fn test(config: &ResolvedMagicPushConfig) -> Self {
        Self {
            title: "Pebble MagicPush test".to_string(),
            content: "Pebble MagicPush test notification".to_string(),
            r#type: "markdown",
            url: config.public_url.clone(),
        }
    }
}

pub fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn format_sender(message: &PushMessage) -> String {
    if message.from_name.trim().is_empty() {
        message.from_address.clone()
    } else {
        format!("{} <{}>", message.from_name, message.from_address)
    }
}

fn format_addresses(addresses: &[PushEmailAddress]) -> String {
    if addresses.is_empty() {
        return "-".to_string();
    }
    addresses
        .iter()
        .map(|addr| match addr.name.as_deref().map(str::trim) {
            Some("") | None => addr.address.clone(),
            Some(name) => format!("{name} <{}>", addr.address),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_mail_time(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn truncate_body(body: &str) -> String {
    const MAX_LEN: usize = 500;
    let trimmed = body.trim();
    let mut end = 0;
    for (idx, _) in trimmed.char_indices() {
        if idx > MAX_LEN {
            break;
        }
        end = idx;
    }
    if trimmed.len() <= MAX_LEN {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..end])
    }
}
