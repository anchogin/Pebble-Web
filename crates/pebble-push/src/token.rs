use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TokenCodecError {
    #[error("Failed to encrypt MagicPush token: {0}")]
    Encrypt(String),
    #[error("Invalid MagicPush token hex: {0}")]
    InvalidHex(hex::FromHexError),
    #[error("Failed to decrypt MagicPush token: {0}")]
    Decrypt(String),
    #[error("MagicPush token is not UTF-8: {0}")]
    Utf8(std::string::FromUtf8Error),
}

pub fn encrypt_magicpush_token<F, E>(token: &str, encrypt: F) -> Result<String, TokenCodecError>
where
    F: FnOnce(&[u8]) -> Result<Vec<u8>, E>,
    E: Display,
{
    let encrypted =
        encrypt(token.as_bytes()).map_err(|error| TokenCodecError::Encrypt(error.to_string()))?;
    Ok(hex::encode(encrypted))
}

pub fn decrypt_magicpush_token<F, E>(
    encrypted_hex: &str,
    decrypt: F,
) -> Result<String, TokenCodecError>
where
    F: FnOnce(&[u8]) -> Result<Vec<u8>, E>,
    E: Display,
{
    let encrypted = hex::decode(encrypted_hex).map_err(TokenCodecError::InvalidHex)?;
    let decrypted =
        decrypt(&encrypted).map_err(|error| TokenCodecError::Decrypt(error.to_string()))?;
    String::from_utf8(decrypted).map_err(TokenCodecError::Utf8)
}
