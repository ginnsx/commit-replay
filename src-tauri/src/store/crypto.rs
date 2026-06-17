use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

fn derive_key() -> [u8; 32] {
    let machine = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "relay".into());
    let mut hasher = Sha256::new();
    hasher.update(b"relay-copy-diff-v1:");
    hasher.update(machine.as_bytes());
    hasher.finalize().into()
}

pub fn encrypt_secret(plain: &str) -> Result<String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::Other(anyhow::anyhow!("cipher init: {e}")))?;
    let nonce_bytes: [u8; 12] = rand_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|e| AppError::Other(anyhow::anyhow!("encrypt: {e}")))?;
    Ok(format!(
        "{}{}",
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

pub fn decrypt_secret(encoded: &str) -> Result<String> {
    if encoded.is_empty() {
        return Ok(String::new());
    }
    if encoded.len() < 24 {
        return Err(AppError::Other(anyhow::anyhow!("invalid encrypted secret")));
    }
    let (nonce_hex, cipher_hex) = encoded.split_at(24);
    let nonce_bytes = hex::decode(nonce_hex)
        .map_err(|e| AppError::Other(anyhow::anyhow!("nonce decode: {e}")))?;
    let ciphertext = hex::decode(cipher_hex)
        .map_err(|e| AppError::Other(anyhow::anyhow!("cipher decode: {e}")))?;
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::Other(anyhow::anyhow!("cipher init: {e}")))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plain = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| AppError::Other(anyhow::anyhow!("decrypt: {e}")))?;
    String::from_utf8(plain).map_err(|e| AppError::Other(anyhow::anyhow!("utf8: {e}")))
}

fn rand_nonce() -> [u8; 12] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut nonce = [0u8; 12];
    let bytes = (nanos as u64).to_le_bytes();
    nonce[..8].copy_from_slice(&bytes);
    nonce[8..12].copy_from_slice(&bytes[..4]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_roundtrip() {
        let enc = encrypt_secret("secret").unwrap();
        assert!(!enc.is_empty());
        assert_eq!(decrypt_secret(&enc).unwrap(), "secret");
    }
}
