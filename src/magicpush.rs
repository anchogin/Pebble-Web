use pebble_core::{MagicPushConfigRecord, Message};
use pebble_crypto::CryptoService;
use pebble_mail::StoredMessage;
use pebble_push::{
    resolve_magicpush_config, MagicPushClient, PushEmailAddress, PushMessage, StoredMagicPushConfig,
};
use pebble_store::Store;
use std::sync::Arc;

pub use pebble_push::ResolvedMagicPushConfig;

#[derive(Clone)]
pub struct MagicPushNotifier {
    store: Arc<Store>,
    crypto: Arc<CryptoService>,
    client: MagicPushClient,
}

impl MagicPushNotifier {
    pub fn new(store: Arc<Store>, crypto: Arc<CryptoService>) -> Result<Self, String> {
        Ok(Self {
            store,
            crypto,
            client: MagicPushClient::new().map_err(|error| error.to_string())?,
        })
    }

    pub async fn send_stored_message(&self, stored: &StoredMessage) -> Result<bool, String> {
        if !stored.notify {
            return Ok(false);
        }
        let Some(config) = self.resolve_stored_config()? else {
            return Ok(false);
        };
        self.client
            .send_message(&push_message_from_core(&stored.message), &config)
            .await
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub async fn send_test_push(&self, config: &ResolvedMagicPushConfig) -> Result<(), String> {
        self.client
            .send_test_push(config)
            .await
            .map_err(|error| error.to_string())
    }

    fn resolve_stored_config(&self) -> Result<Option<ResolvedMagicPushConfig>, String> {
        let Some(record) = self
            .store
            .get_magicpush_config()
            .map_err(|e| format!("Failed to load MagicPush config: {e}"))?
        else {
            return Ok(None);
        };
        let Some(token_encrypted) = record.token_encrypted.as_deref().and_then(non_empty) else {
            return Ok(None);
        };
        let token = decrypt_magicpush_token(&self.crypto, token_encrypted)?;
        let Some(public_url) = shared_public_url(self.store.as_ref())
            .map_err(|e| format!("Failed to load app settings: {e}"))?
        else {
            return Ok(None);
        };
        Ok(resolve_magicpush_config(
            &stored_config_from_record(&record, public_url),
            token,
        ))
    }
}

pub fn encrypt_magicpush_token(crypto: &CryptoService, token: &str) -> Result<String, String> {
    pebble_push::encrypt_magicpush_token(token, |plaintext| crypto.encrypt(plaintext))
        .map_err(|error| error.to_string())
}

pub fn decrypt_magicpush_token(
    crypto: &CryptoService,
    encrypted_hex: &str,
) -> Result<String, String> {
    pebble_push::decrypt_magicpush_token(encrypted_hex, |ciphertext| crypto.decrypt(ciphertext))
        .map_err(|error| error.to_string())
}

pub fn resolve_magicpush_config_record(
    record: &MagicPushConfigRecord,
    public_url: String,
    token: String,
) -> Option<ResolvedMagicPushConfig> {
    resolve_magicpush_config(&stored_config_from_record(record, public_url), token)
}

pub fn shared_public_url(store: &Store) -> pebble_core::Result<Option<String>> {
    Ok(non_empty(&store.get_app_settings()?.public_url).map(str::to_string))
}

fn stored_config_from_record(
    record: &MagicPushConfigRecord,
    public_url: String,
) -> StoredMagicPushConfig {
    StoredMagicPushConfig {
        base_url: record.base_url.clone(),
        token_encrypted: record.token_encrypted.clone(),
        public_url,
        is_enabled: record.is_enabled,
    }
}

fn push_message_from_core(message: &Message) -> PushMessage {
    PushMessage {
        id: message.id.clone(),
        subject: message.subject.clone(),
        snippet: message.snippet.clone(),
        from_address: message.from_address.clone(),
        from_name: message.from_name.clone(),
        to_list: message
            .to_list
            .iter()
            .map(|address| PushEmailAddress {
                name: address.name.clone(),
                address: address.address.clone(),
            })
            .collect(),
        body_text: message.body_text.clone(),
        date: message.date,
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
