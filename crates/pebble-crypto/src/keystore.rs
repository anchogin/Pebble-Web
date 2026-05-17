use pebble_core::{PebbleError, Result};
use rand::RngCore;
use std::fs;
use std::path::Path;
use tracing::info;
use zeroize::Zeroizing;

pub struct KeyStore;

impl KeyStore {
    /// Load DEK from environment variable PEBBLE_ENCRYPTION_KEY (hex-encoded 32 bytes),
    /// or from a file at the given path. If neither exists, generate and save to file.
    pub fn get_or_create_dek(key_file_path: Option<&Path>) -> Result<Zeroizing<[u8; 32]>> {
        // Priority 1: environment variable
        if let Ok(hex_key) = std::env::var("PEBBLE_ENCRYPTION_KEY") {
            let bytes = hex::decode(hex_key.trim()).map_err(|e| {
                PebbleError::Auth(format!("Invalid PEBBLE_ENCRYPTION_KEY hex: {e}"))
            })?;
            if bytes.len() != 32 {
                return Err(PebbleError::Auth(format!(
                    "PEBBLE_ENCRYPTION_KEY must be 32 bytes, got {}",
                    bytes.len()
                )));
            }
            let mut key = Zeroizing::new([0u8; 32]);
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        // Priority 2: key file
        let key_path = key_file_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new("/data/encryption.key").to_path_buf());

        if key_path.exists() {
            let hex_key = fs::read_to_string(&key_path)
                .map_err(|e| PebbleError::Auth(format!("Failed to read key file: {e}")))?;
            let bytes = hex::decode(hex_key.trim())
                .map_err(|e| PebbleError::Auth(format!("Invalid key file hex: {e}")))?;
            if bytes.len() != 32 {
                return Err(PebbleError::Auth(format!(
                    "Key file must contain 32 bytes, got {}",
                    bytes.len()
                )));
            }
            let mut key = Zeroizing::new([0u8; 32]);
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        // Generate new key and save to file
        info!("No DEK found, generating new one at {:?}", key_path);
        let mut key = Zeroizing::new([0u8; 32]);
        rand::thread_rng().fill_bytes(&mut *key);

        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| PebbleError::Auth(format!("Failed to create key dir: {e}")))?;
        }
        fs::write(&key_path, hex::encode(*key))
            .map_err(|e| PebbleError::Auth(format!("Failed to write key file: {e}")))?;

        Ok(key)
    }
}
