use crate::types::{ResolvedMagicPushConfig, StoredMagicPushConfig};

pub fn resolve_magicpush_config(
    record: &StoredMagicPushConfig,
    token: String,
) -> Option<ResolvedMagicPushConfig> {
    if !record.is_enabled || token.trim().is_empty() {
        return None;
    }
    Some(ResolvedMagicPushConfig {
        base_url: non_empty(&record.base_url).map(trim_trailing_slash)?,
        token,
        public_url: non_empty(&record.public_url).map(trim_trailing_slash)?,
    })
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}
