//! AES-256-GCM helpers compatible with the proxy's credential store format.
//!
//! The proxy (`src/auth/store.ts`) encrypts `JSON.stringify(credential)` with
//! AES-256-GCM using `SHA-256(secret)` as the key, a random 12-byte IV and
//! base64(iv || ciphertext+tag). We replicate that exactly so the manager can
//! write credentials the proxy reads, and read credentials it wrote.

use aes_gcm::aead::{Aead, KeyInit, OsRng as AesOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use sha2::{Digest, Sha256};

/// 12-byte GCM IV, matching the proxy.
const IV_LEN: usize = 12;

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// SHA-256(secret) — same derivation as the proxy store.
pub fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

/// Vault key derivation (distinct domain from the proxy store key).
pub fn derive_vault_key(master_secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"zcm-vault-v1:");
    hasher.update(master_secret.as_bytes());
    hasher.finalize().into()
}

/// Encrypt plaintext -> base64(iv || ct+tag). Matches proxy store encoding.
pub fn seal(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut iv = [0u8; IV_LEN];
    use aes_gcm::aead::rand_core::RngCore;
    AesOsRng.fill_bytes(&mut iv);
    let ct = cipher
        .encrypt(Nonce::from_slice(&iv), plaintext.as_bytes())
        .map_err(|e| format!("encrypt failed: {e}"))?;
    let mut combined = Vec::with_capacity(IV_LEN + ct.len());
    combined.extend_from_slice(&iv);
    combined.extend_from_slice(&ct);
    Ok(b64().encode(combined))
}

/// Decrypt base64(iv || ct+tag) -> plaintext.
pub fn open(key: &[u8; 32], encoded: &str) -> Result<String, String> {
    let combined = b64()
        .decode(encoded.trim())
        .map_err(|e| format!("base64 decode failed: {e}"))?;
    if combined.len() <= IV_LEN {
        return Err("ciphertext too short".into());
    }
    let (iv, ct) = combined.split_at(IV_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let pt = cipher
        .decrypt(Nonce::from_slice(iv), ct)
        .map_err(|_| "decryption failed (wrong key or corrupted data)".to_string())?;
    String::from_utf8(pt).map_err(|e| format!("invalid utf-8: {e}"))
}

pub fn random_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = derive_key("secret-1");
        let sealed = seal(&key, r#"{"apiKey":"abc"}"#).unwrap();
        assert_eq!(open(&key, &sealed).unwrap(), r#"{"apiKey":"abc"}"#);
    }

    #[test]
    fn wrong_key_fails() {
        let key = derive_key("secret-1");
        let sealed = seal(&key, "hello").unwrap();
        assert!(open(&derive_key("secret-2"), &sealed).is_err());
    }

    #[test]
    fn vault_key_differs_from_store_key() {
        assert_ne!(derive_key("m"), derive_vault_key("m"));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(random_token(), random_token());
    }
}
