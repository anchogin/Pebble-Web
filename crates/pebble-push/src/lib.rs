mod client;
mod config;
mod payload;
mod token;
mod types;

pub use client::{MagicPushClient, MagicPushError};
pub use config::resolve_magicpush_config;
pub use payload::{percent_encode, MagicPushPayload};
pub use token::{decrypt_magicpush_token, encrypt_magicpush_token, TokenCodecError};
pub use types::{PushEmailAddress, PushMessage, ResolvedMagicPushConfig, StoredMagicPushConfig};
