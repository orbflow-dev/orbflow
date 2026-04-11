// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AES-256-GCM encryption/decryption for credential storage.
//!
//! Wire format: `nonce (12 bytes) || ciphertext || tag (16 bytes)`
//! This is byte-compatible with the Go implementation.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngExt;

use crate::error::OrbflowError;

const NONCE_SIZE: usize = 12;

/// Encrypts plaintext using AES-256-GCM.
///
/// The key must be exactly 32 bytes. Returns `nonce || ciphertext || tag`.
pub fn encrypt(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, OrbflowError> {
    if key.len() != 32 {
        return Err(OrbflowError::Crypto(format!(
            "invalid key length: expected 32 bytes, got {}",
            key.len()
        )));
    }

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| OrbflowError::Crypto(e.to_string()))?;

    let nonce_bytes: [u8; NONCE_SIZE] = rand::rng().random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| OrbflowError::Crypto(e.to_string()))?;

    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypts ciphertext produced by [`encrypt`].
///
/// The key must be exactly 32 bytes. Input format: `nonce (12) || ciphertext || tag (16)`.
pub fn decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, OrbflowError> {
    if key.len() != 32 {
        return Err(OrbflowError::Crypto(format!(
            "invalid key length: expected 32 bytes, got {}",
            key.len()
        )));
    }

    if data.len() < NONCE_SIZE + 16 {
        return Err(OrbflowError::Crypto(
            "ciphertext too short (must contain nonce + tag)".into(),
        ));
    }

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| OrbflowError::Crypto(e.to_string()))?;

    let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
    let ciphertext = &data[NONCE_SIZE..];

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| OrbflowError::Crypto(e.to_string()))
}

/// Constant-time byte comparison to prevent timing attacks on token validation.
///
/// Returns `true` only when both slices have equal length and identical content.
/// Delegates to the `subtle` crate for compiler-guaranteed constant-time behavior.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_key() -> Vec<u8> {
        let key: [u8; 32] = rand::rng().random();
        key.to_vec()
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = valid_key();
        let plaintext = b"hello world";
        let encrypted = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key() {
        let key1 = valid_key();
        let key2 = valid_key();
        let encrypted = encrypt(&key1, b"secret").unwrap();
        assert!(decrypt(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_truncated_ciphertext() {
        let key = valid_key();
        let encrypted = encrypt(&key, b"hello").unwrap();
        assert!(decrypt(&key, &encrypted[..10]).is_err());
    }

    #[test]
    fn test_empty_plaintext() {
        let key = valid_key();
        let encrypted = encrypt(&key, b"").unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn test_nonce_uniqueness() {
        let key = valid_key();
        let e1 = encrypt(&key, b"test").unwrap();
        let e2 = encrypt(&key, b"test").unwrap();
        // Nonces (first 12 bytes) should differ
        assert_ne!(&e1[..NONCE_SIZE], &e2[..NONCE_SIZE]);
    }

    #[test]
    fn test_invalid_key_length() {
        assert!(encrypt(&[0u8; 16], b"test").is_err());
        assert!(encrypt(&[0u8; 48], b"test").is_err());
        assert!(decrypt(&[0u8; 16], &[0u8; 30]).is_err());
    }

    #[test]
    fn test_tampered_ciphertext() {
        let key = valid_key();
        let mut encrypted = encrypt(&key, b"hello").unwrap();
        // Flip a byte in the ciphertext portion
        let idx = NONCE_SIZE + 1;
        encrypted[idx] ^= 0xFF;
        assert!(decrypt(&key, &encrypted).is_err());
    }

    #[test]
    fn test_constant_time_eq_equal() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_constant_time_eq_different_content() {
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    #[test]
    fn test_constant_time_eq_different_length() {
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"hi", b"hello"));
    }
}
