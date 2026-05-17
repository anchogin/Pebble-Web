pub mod aes;
pub mod keystore;

use pebble_core::Result;
use std::path::Path;
use zeroize::Zeroizing;

pub struct CryptoService {
    dek: Zeroizing<[u8; 32]>,
}

impl CryptoService {
    /// Initialize by loading (or creating) the DEK from env or file.
    pub fn init(key_file_path: Option<&Path>) -> Result<Self> {
        let dek = keystore::KeyStore::get_or_create_dek(key_file_path)?;
        Ok(Self { dek })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        aes::encrypt(&self.dek, plaintext)
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        aes::decrypt(&self.dek, ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_crypto_service_init_with_env() {
        let _guard = encryption_env_lock().lock().unwrap();
        let key = [0xABu8; 32];
        env::set_var("PEBBLE_ENCRYPTION_KEY", hex::encode(key));
        let service = CryptoService::init(None).unwrap();
        let encrypted = service.encrypt(b"hello").unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, b"hello");
        env::remove_var("PEBBLE_ENCRYPTION_KEY");
    }

    #[test]
    fn test_crypto_service_init_generates_file() {
        let _guard = encryption_env_lock().lock().unwrap();
        env::remove_var("PEBBLE_ENCRYPTION_KEY");
        let tmp = TempDir::new().unwrap();
        let key_path = tmp.path().join("encryption.key");
        let service = CryptoService::init(Some(&key_path)).unwrap();
        assert!(key_path.exists());
        let encrypted = service.encrypt(b"test").unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, b"test");
    }

    fn encryption_env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }
}
