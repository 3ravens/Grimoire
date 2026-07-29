/// Grimoire cryptographic primitives.
///
/// All encryption uses AES-256-GCM (authenticated, so we detect tampering).
/// Keys are derived from passwords using Argon2id (memory-hard, recommended
/// by OWASP for password hashing/KDF as of 2024).
///
/// Storage format for ciphertext blobs (stored as base64 TEXT in SQLite):
///   [12 bytes nonce][ciphertext][16 bytes GCM auth tag]
///
/// The salt is stored separately in plaintext alongside the ciphertext — salts
/// are not secret; their job is to make identical passwords produce different keys.
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

/// A known plaintext we encrypt to verify a password guess without storing the password.
/// If we can decrypt the sentinel and get back this value, the password is correct.
const SENTINEL_PLAINTEXT: &[u8] = b"grimoire_ok";

/// Generate a cryptographically-random 32-byte salt.
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive a 256-bit encryption key from `password` and `salt` using Argon2id.
///
/// Parameters (OWASP recommended minimum as of 2024):
///   m = 65536 KiB (64 MiB memory), t = 2 iterations, p = 1 lane
///
/// This takes ~100ms on a modern machine — acceptable for a one-time unlock,
/// slow enough to make brute-force attacks costly.
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(65_536, 2, 1, Some(32)).map_err(|e| format!("argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("argon2 key derivation: {e}"))?;
    Ok(key)
}

/// Encrypt `plaintext` with `key` using AES-256-GCM.
///
/// Returns: `nonce (12 bytes) || ciphertext || auth_tag (16 bytes)`, base64-encoded.
/// The nonce is randomly generated per call, so encrypting the same plaintext
/// twice produces different output — this is intentional and required.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> String {
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut ciphertext = cipher
        .encrypt(nonce, plaintext)
        .expect("AES-GCM encryption failed");

    // Prepend nonce to ciphertext so we have a single self-contained blob.
    let mut blob = nonce_bytes.to_vec();
    blob.append(&mut ciphertext);

    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&blob)
}

/// Decrypt a base64 blob produced by `encrypt`.
///
/// Returns `Err` if the blob is malformed, the nonce is wrong length, or the
/// GCM auth tag doesn't match (indicating wrong key or tampered data).
pub fn decrypt(key: &[u8; 32], blob_b64: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let blob = base64::engine::general_purpose::STANDARD
        .decode(blob_b64)
        .map_err(|e| format!("base64 decode failed: {e}"))?;

    if blob.len() < 12 {
        return Err("ciphertext blob too short".to_string());
    }

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "decryption failed — wrong key or corrupted data".to_string())
}

/// Produce a sentinel blob: encrypt the known plaintext with `key`.
/// Store this alongside the salt so we can later verify a password attempt.
pub fn make_sentinel(key: &[u8; 32]) -> String {
    encrypt(key, SENTINEL_PLAINTEXT)
}

/// Return true if `key` correctly decrypts `sentinel` back to the known plaintext.
pub fn verify_sentinel(key: &[u8; 32], sentinel_b64: &str) -> bool {
    match decrypt(key, sentinel_b64) {
        Ok(plaintext) => plaintext == SENTINEL_PLAINTEXT,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let salt = [3u8; 32];
        let key = derive_key("correct horse battery staple", &salt).expect("derive");
        let blob = encrypt(&key, b"secret payload");
        let plain = decrypt(&key, &blob).unwrap();
        assert_eq!(plain, b"secret payload");
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let salt = [9u8; 32];
        let k1 = derive_key("password-one", &salt).unwrap();
        let k2 = derive_key("password-two", &salt).unwrap();
        let blob = encrypt(&k1, b"data");
        assert!(decrypt(&k2, &blob).is_err());
    }

    #[test]
    fn tampered_blob_fails_decrypt() {
        let salt = [1u8; 32];
        let key = derive_key("pw", &salt).unwrap();
        let mut blob = encrypt(&key, b"x");
        let bytes = base64::engine::general_purpose::STANDARD.decode(&blob).unwrap();
        let mut v = bytes.clone();
        if let Some(last) = v.last_mut() {
            *last ^= 0xFF;
        }
        blob = base64::engine::general_purpose::STANDARD.encode(v);
        assert!(decrypt(&key, &blob).is_err());
    }

    #[test]
    fn sentinel_round_trip() {
        let salt = [5u8; 32];
        let key = derive_key("vault-password", &salt).unwrap();
        let s = make_sentinel(&key);
        assert!(verify_sentinel(&key, &s));
        let other = derive_key("other", &salt).unwrap();
        assert!(!verify_sentinel(&other, &s));
    }
}


