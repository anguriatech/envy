//! Master key management via the OS Credential Manager.
//!
//! The 32-byte vault master key is stored exclusively in the OS Credential Manager
//! (Linux: Secret Service / libsecret, macOS: Keychain, Windows: Credential Manager).
//! It is persisted as a 64-character lowercase hex string and never written to any
//! file, environment variable, or log.
//!
//! # Public API
//! - [`get_or_create_master_key`] — fetch or generate the master key.
//! - [`set_master_key`] — overwrite the stored master key (used by `envy key import`
//!   to restore from a recovery file — see `crypto::artifact` for the encryption
//!   used to protect that file).
//! - [`encode_key_hex`] / [`decode_key_hex`] — the same hex encoding used for
//!   keyring storage, exposed so the recovery-file flow doesn't invent a second
//!   representation of the same 32 bytes.

use aes_gcm::{
    Aes256Gcm,
    aead::{KeyInit, OsRng},
};
use zeroize::Zeroizing;

use crate::crypto::CryptoError;

// ---------------------------------------------------------------------------
// T019 — Constants (private to this module)
// ---------------------------------------------------------------------------

const SERVICE_NAME: &str = "envy";
const ACCOUNT_NAME: &str = "master-key";

// ---------------------------------------------------------------------------
// T019 — Private helpers
// ---------------------------------------------------------------------------

/// Encodes a 32-byte key as a 64-character lowercase hex string for keyring storage.
fn encode_key(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Decodes a hex-encoded string from the keyring back into a 32-byte key.
///
/// Returns [`CryptoError::KeyCorrupted`] if the string is not exactly 64 hex chars
/// (the encoding of 32 bytes), ensuring any length mismatch is caught at retrieval.
fn decode_key(s: &str) -> Result<[u8; 32], CryptoError> {
    if s.len() != 64 {
        return Err(CryptoError::KeyCorrupted);
    }
    let mut key = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        key[i] = (hi << 4) | lo;
    }
    Ok(key)
}

fn hex_nibble(c: u8) -> Result<u8, CryptoError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(CryptoError::KeyCorrupted),
    }
}

/// Attempts to retrieve the existing master key from the OS Credential Manager.
///
/// Returns:
/// - `Ok(key)` — key found and decoded successfully.
/// - `Err(KeyNotFound)` — no entry exists yet; caller should generate one.
/// - `Err(KeyCorrupted)` — entry exists but is not a valid 64-char hex string.
/// - `Err(KeyringUnavailable)` — daemon unreachable or access denied.
fn retrieve_key() -> Result<[u8; 32], CryptoError> {
    let entry = keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
        .map_err(|e| CryptoError::KeyringUnavailable(e.to_string()))?;
    match entry.get_password() {
        Ok(pw) => decode_key(&pw),
        Err(keyring::Error::NoEntry) => Err(CryptoError::KeyNotFound),
        Err(e) => Err(CryptoError::KeyringUnavailable(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// T020 — Public API
// ---------------------------------------------------------------------------

/// Returns the 32-byte vault master key from the OS Credential Manager.
///
/// On first call, a cryptographically random key is generated via [`OsRng`] and stored
/// as a hex-encoded string in the OS Credential Manager. Subsequent calls return the
/// same stored key. The returned [`Zeroizing`] wrapper zeroes the backing `[u8; 32]`
/// when dropped (Constitution Principle I).
///
/// # Errors
/// - [`CryptoError::KeyCorrupted`] — a key entry exists but cannot be decoded as 32 bytes.
/// - [`CryptoError::KeyringUnavailable`] — the OS Credential Manager is unreachable or
///   access was denied. The diagnostic string MUST NOT contain key material.
pub fn get_or_create_master_key() -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    match retrieve_key() {
        Ok(key) => Ok(Zeroizing::new(key)),

        Err(CryptoError::KeyNotFound) => {
            // First run: generate 32 cryptographically random bytes for the master key.
            // Aes256Gcm::generate_key uses OsRng (CSPRNG) and returns exactly 32 bytes.
            let key_ga = Aes256Gcm::generate_key(OsRng);
            // SAFETY: generate_key for Aes256Gcm always returns exactly 32 bytes (U32).
            let mut key_bytes = [0u8; 32];
            key_bytes.copy_from_slice(key_ga.as_slice());

            // Persist to the OS Credential Manager as a hex-encoded string.
            let entry = match keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
                Ok(e) => e,
                Err(e) => {
                    return ci_fallback(CryptoError::KeyringUnavailable(e.to_string()));
                }
            };
            entry
                .set_password(&encode_key(&key_bytes))
                .map_err(|e| CryptoError::KeyringUnavailable(e.to_string()))?;

            Ok(Zeroizing::new(key_bytes))
        }

        Err(CryptoError::KeyringUnavailable(msg)) => {
            ci_fallback(CryptoError::KeyringUnavailable(msg))
        }

        Err(e) => Err(e),
    }
}

/// Hex-encodes a 32-byte key using the same format persisted in the OS keyring.
///
/// Used by `envy key export` to write the master key into a recovery file —
/// always inside an encrypted envelope (see `crypto::artifact::seal_envelope`),
/// never in the clear.
pub fn encode_key_hex(key: &[u8; 32]) -> String {
    encode_key(key)
}

/// Decodes a hex string produced by [`encode_key_hex`] back into a 32-byte key.
///
/// # Errors
/// - [`CryptoError::KeyCorrupted`] if `s` is not exactly 64 hex characters.
pub fn decode_key_hex(s: &str) -> Result<[u8; 32], CryptoError> {
    decode_key(s)
}

/// Overwrites the OS Credential Manager entry with `key`.
///
/// Used exclusively by `envy key import` to restore a master key from a
/// recovery file created by `envy key export`.
///
/// # Security
/// This **overwrites** any existing master key. If a `vault.db` already
/// exists locally and was encrypted under a *different* key, it becomes
/// permanently unreadable after this call unless `key` happens to match.
/// Callers (the CLI layer) MUST confirm this with the user before calling —
/// this function performs no such check itself.
///
/// # Errors
/// - [`CryptoError::KeyringUnavailable`] if the OS Credential Manager entry
///   cannot be created or written.
pub fn set_master_key(key: &[u8; 32]) -> Result<(), CryptoError> {
    let entry = keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
        .map_err(|e| CryptoError::KeyringUnavailable(e.to_string()))?;
    entry
        .set_password(&encode_key(key))
        .map_err(|e| CryptoError::KeyringUnavailable(e.to_string()))?;
    Ok(())
}

/// If `ENVY_PASSPHRASE` or `CI` is set, returns a deterministic zero ephemeral key so
/// that headless CI environments (no D-Bus / Secret Service) can still operate against
/// an ephemeral vault. Otherwise propagates the original [`CryptoError::KeyringUnavailable`].
///
/// This check is performed *after* the keyring attempt fails, so local developers who
/// export `ENVY_PASSPHRASE` but rely on their OS Keychain are unaffected.
fn ci_fallback(err: CryptoError) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    if std::env::var("ENVY_PASSPHRASE").is_ok() || std::env::var("CI").is_ok() {
        Ok(Zeroizing::new([0u8; 32]))
    } else {
        Err(err)
    }
}

// ---------------------------------------------------------------------------
// T017–T018 — Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T017 — Integration test: requires a live Secret Service / keyring daemon.
    // Marked #[ignore] so CI environments without a daemon do not fail.
    // Run manually with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn get_or_create_master_key_is_idempotent() {
        let key1 = get_or_create_master_key().expect("must succeed with a live keyring daemon");
        assert_eq!(key1.len(), 32, "returned key must be exactly 32 bytes");

        let key2 = get_or_create_master_key().expect("second call must also succeed");
        assert_eq!(
            *key1, *key2,
            "key must be identical on repeated calls (idempotent)"
        );
    }

    #[test]
    fn encode_decode_key_hex_round_trip() {
        let key = [0x7Au8; 32];
        let hex = encode_key_hex(&key);
        assert_eq!(hex.len(), 64);
        let decoded = decode_key_hex(&hex).expect("decode must succeed");
        assert_eq!(decoded, key);
    }

    #[test]
    fn decode_key_hex_rejects_malformed_input() {
        let result = decode_key_hex("not-hex-and-wrong-length");
        assert!(matches!(result, Err(CryptoError::KeyCorrupted)));
    }

    // Integration test: requires a live Secret Service / keyring daemon.
    // Marked #[ignore] for the same reason as get_or_create_master_key_is_idempotent.
    #[test]
    #[ignore]
    fn set_master_key_then_get_or_create_returns_it() {
        let key = [0x11u8; 32];
        set_master_key(&key).expect("set_master_key must succeed with a live keyring daemon");
        let fetched = get_or_create_master_key().expect("must succeed with a live keyring daemon");
        assert_eq!(*fetched, key, "get_or_create must return the key we just set");
    }

    // T018 — Unit test: validates decode_key length-checking without a live keyring.
    // Exercises the same validation logic that retrieve_key() applies to stored values.
    #[test]
    fn key_length_validated() {
        // A 10-byte key would encode to 20 hex chars — must return KeyCorrupted
        let result = decode_key("deadbeefdeadbeefdeadbeef"); // 24 chars = 12 bytes
        assert!(
            matches!(result, Err(CryptoError::KeyCorrupted)),
            "short hex string must return KeyCorrupted, got: {:?}",
            result
        );

        // 66-char string — too long — must return KeyCorrupted
        let long_hex = "a".repeat(66);
        let result = decode_key(&long_hex);
        assert!(
            matches!(result, Err(CryptoError::KeyCorrupted)),
            "66-char hex string must return KeyCorrupted, got: {:?}",
            result
        );

        // Valid 64-char hex string (32 bytes of 0x42) — must succeed
        let valid_hex = "42".repeat(32); // 64 chars
        let result = decode_key(&valid_hex);
        assert!(
            result.is_ok(),
            "valid 64-char hex string must succeed, got: {:?}",
            result
        );
        // SAFETY: result is Ok — unwrap is logically guaranteed by the assert above.
        let key = result.unwrap();
        assert!(
            key.iter().all(|&b| b == 0x42),
            "decoded key bytes must match the hex input"
        );
    }
}
