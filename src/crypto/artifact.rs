//! GitOps sync artifact cryptography — `envy.enc` generation and parsing.
//!
//! Provides the low-level cryptographic primitives for the `envy.enc` artifact:
//! Argon2id key derivation, AES-256-GCM per-environment encryption, and the
//! self-describing envelope data types that make up the artifact's JSON structure.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::db`.
//! - MUST NOT perform file I/O. All file operations live in `crate::core::sync`.

use std::collections::BTreeMap;

use argon2::{Argon2, Params, Version};
use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// T012 — Constants and ArtifactError
// ---------------------------------------------------------------------------

/// Schema version embedded in every `envy.enc` file.
/// Bump this when the JSON structure changes in a backward-incompatible way.
pub const ARTIFACT_VERSION: u32 = 1;

/// Argon2id memory cost in kibibytes (64 MiB).
pub const KDF_MEMORY_KIB: u32 = 65536;
/// Argon2id time cost (iterations).
pub const KDF_TIME_COST: u32 = 3;
/// Argon2id parallelism factor.
pub const KDF_PARALLELISM: u32 = 4;
/// Size in bytes of the per-envelope random salt.
pub const KDF_SALT_BYTES: usize = 16;

/// All errors that can be returned by the artifact cryptography layer.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    /// Passphrase is empty or whitespace-only.
    #[error("passphrase must not be empty or whitespace")]
    WeakPassphrase,

    /// The top-level `envy.enc` JSON structure is invalid.
    #[error("envy.enc is malformed: {0}")]
    MalformedArtifact(String),

    /// An individual envelope entry is structurally invalid or failed authentication.
    ///
    /// Both "wrong passphrase" and "tampered ciphertext" map here — AES-GCM cannot
    /// distinguish the two cases. The caller (`unseal_artifact`) MUST treat this as a
    /// graceful skip, never as a hard error (Progressive Disclosure contract).
    #[error("envelope for environment '{0}' is malformed: {1}")]
    MalformedEnvelope(String, String),

    /// The artifact uses an unknown schema version or an unsupported KDF algorithm.
    #[error("unsupported artifact version {0} (expected {1})")]
    UnsupportedVersion(u32, u32),

    /// Argon2id key derivation failed (internal error).
    #[error("key derivation failed: {0}")]
    KdfFailed(String),
}

// ---------------------------------------------------------------------------
// T013 — Data types
// ---------------------------------------------------------------------------

/// Root structure of an `envy.enc` file.
///
/// `environments` uses [`BTreeMap`] to guarantee alphabetical JSON key ordering,
/// which produces stable Git diffs when individual environments change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncArtifact {
    /// Schema version. MUST equal [`ARTIFACT_VERSION`] for this release.
    pub version: u32,
    /// Per-environment encrypted payloads, keyed by environment name.
    pub environments: BTreeMap<String, EncryptedEnvelope>,
}

/// Self-describing encrypted payload for one environment.
///
/// Every field needed to decrypt the payload given the correct passphrase is
/// embedded here — no external metadata is required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    /// Base64-encoded AES-256-GCM ciphertext with the 16-byte GCM tag appended.
    pub ciphertext: String,
    /// Base64-encoded 12-byte (96-bit) AES-GCM nonce. Unique per envelope.
    pub nonce: String,
    /// Argon2id parameters used to derive the AES-256-GCM key from the passphrase.
    pub kdf: KdfParams,
}

/// Argon2id key-derivation parameters embedded in every [`EncryptedEnvelope`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    /// KDF algorithm identifier. Always `"argon2id"` for this version.
    pub algorithm: String,
    /// Argon2 memory parameter in kibibytes.
    pub memory_kib: u32,
    /// Argon2 time cost (sequential iterations).
    pub time_cost: u32,
    /// Argon2 parallelism factor (number of lanes).
    pub parallelism: u32,
    /// Base64-encoded 16-byte random salt, unique per envelope per seal operation.
    pub salt: String,
}

/// Ephemeral plaintext content for one environment.
///
/// This struct is serialized to JSON bytes and then encrypted by [`seal_envelope`].
/// It is NEVER written to disk in plaintext form.
///
/// Values are [`Zeroizing<String>`] so backing memory is zeroed when dropped
/// (Constitution Principle I).
pub struct ArtifactPayload {
    /// Secret key names mapped to their plaintext values.
    pub secrets: BTreeMap<String, Zeroizing<String>>,
}

// ---------------------------------------------------------------------------
// T014 — derive_key
// ---------------------------------------------------------------------------

/// Derives a 32-byte AES-256-GCM key from `passphrase` using Argon2id.
///
/// # Arguments
/// - `passphrase` — User-provided string. MUST NOT be empty or whitespace-only.
/// - `salt` — 16-byte random salt. MUST be unique per envelope per seal operation.
/// - `params` — Argon2id cost parameters (memory, time, parallelism).
///
/// # Returns
/// `Ok(Zeroizing<[u8; 32]>)` — the derived key, zeroed when dropped.
///
/// # Errors
/// - [`ArtifactError::WeakPassphrase`] if `passphrase.trim().is_empty()`.
/// - [`ArtifactError::KdfFailed`] on internal Argon2 failure.
pub fn derive_key(
    passphrase: &str,
    salt: &[u8; 16],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; 32]>, ArtifactError> {
    if passphrase.trim().is_empty() {
        return Err(ArtifactError::WeakPassphrase);
    }
    let argon2_params = Params::new(
        params.memory_kib,
        params.time_cost,
        params.parallelism,
        Some(32),
    )
    .map_err(|e| ArtifactError::KdfFailed(e.to_string()))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, argon2_params);
    let mut output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, output.as_mut())
        .map_err(|e| ArtifactError::KdfFailed(e.to_string()))?;
    Ok(output)
}

// ---------------------------------------------------------------------------
// T015 — seal_envelope
// ---------------------------------------------------------------------------

/// Encrypts `payload` into a self-describing [`EncryptedEnvelope`].
///
/// Generates a fresh 16-byte random salt (for Argon2id key derivation) and a
/// fresh 12-byte random nonce (for AES-256-GCM) from the OS CSPRNG on every call.
/// Both the salt and all Argon2id cost parameters are embedded in the returned
/// envelope so it is fully self-describing.
///
/// # Errors
/// - [`ArtifactError::WeakPassphrase`] if `passphrase.trim().is_empty()`.
/// - [`ArtifactError::KdfFailed`] on internal Argon2 failure.
/// - [`ArtifactError::MalformedArtifact`] on AES-GCM failure (structurally
///   impossible with valid inputs; treated as an internal error).
pub fn seal_envelope(
    passphrase: &str,
    payload: &ArtifactPayload,
) -> Result<EncryptedEnvelope, ArtifactError> {
    if passphrase.trim().is_empty() {
        return Err(ArtifactError::WeakPassphrase);
    }
    // Generate a fresh random salt for this envelope.
    use aes_gcm::aead::OsRng;
    use aes_gcm::aead::rand_core::RngCore;
    let mut salt = [0u8; KDF_SALT_BYTES];
    OsRng.fill_bytes(&mut salt);

    let kdf = KdfParams {
        algorithm: "argon2id".to_string(),
        memory_kib: KDF_MEMORY_KIB,
        time_cost: KDF_TIME_COST,
        parallelism: KDF_PARALLELISM,
        salt: Base64::encode_string(&salt),
    };
    let key = derive_key(passphrase, &salt, &kdf)?;

    // Serialize the inner payload to JSON bytes (key names inside the ciphertext).
    let plain_map: BTreeMap<&str, &str> = payload
        .secrets
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let json_bytes = serde_json::to_vec(&plain_map)
        .map_err(|e| ArtifactError::MalformedArtifact(e.to_string()))?;

    // Encrypt with the existing AES-256-GCM primitive.
    let encrypted = crate::crypto::encrypt(&key, &json_bytes)
        .map_err(|e| ArtifactError::MalformedArtifact(e.to_string()))?;

    Ok(EncryptedEnvelope {
        ciphertext: Base64::encode_string(&encrypted.ciphertext),
        nonce: Base64::encode_string(&encrypted.nonce),
        kdf,
    })
}

// ---------------------------------------------------------------------------
// T016 — unseal_envelope
// ---------------------------------------------------------------------------

/// Decrypts an [`EncryptedEnvelope`] back into an [`ArtifactPayload`].
///
/// Re-derives the AES-256-GCM key from `passphrase` using the Argon2id parameters
/// embedded in `envelope.kdf`, then decrypts and authenticates the ciphertext.
///
/// # Note on authentication failure
/// If AES-GCM authentication fails (wrong passphrase OR tampered ciphertext), this
/// function returns [`ArtifactError::MalformedEnvelope`]. The caller
/// (`unseal_artifact` in `crate::core::sync`) MUST treat this as a graceful skip —
/// never as a hard error. This is the Progressive Disclosure contract.
///
/// # Errors
/// - [`ArtifactError::WeakPassphrase`] if `passphrase.trim().is_empty()`.
/// - [`ArtifactError::UnsupportedVersion`] if `kdf.algorithm` is not `"argon2id"`.
/// - [`ArtifactError::MalformedEnvelope`] if Base64 decoding, AES-GCM authentication,
///   or JSON deserialization fails.
/// - [`ArtifactError::KdfFailed`] on internal Argon2 failure.
pub fn unseal_envelope(
    passphrase: &str,
    env_name: &str,
    envelope: &EncryptedEnvelope,
) -> Result<ArtifactPayload, ArtifactError> {
    if passphrase.trim().is_empty() {
        return Err(ArtifactError::WeakPassphrase);
    }
    if envelope.kdf.algorithm != "argon2id" {
        return Err(ArtifactError::UnsupportedVersion(0, ARTIFACT_VERSION));
    }

    // Decode salt.
    let salt_bytes = Base64::decode_vec(&envelope.kdf.salt)
        .map_err(|e| ArtifactError::MalformedEnvelope(env_name.to_string(), e.to_string()))?;
    let salt: [u8; 16] = salt_bytes.try_into().map_err(|_| {
        ArtifactError::MalformedEnvelope(env_name.to_string(), "salt must be 16 bytes".to_string())
    })?;

    // Re-derive the key.
    let key = derive_key(passphrase, &salt, &envelope.kdf)?;

    // Decode ciphertext and nonce.
    let ct_bytes = Base64::decode_vec(&envelope.ciphertext)
        .map_err(|e| ArtifactError::MalformedEnvelope(env_name.to_string(), e.to_string()))?;
    let nonce_bytes = Base64::decode_vec(&envelope.nonce)
        .map_err(|e| ArtifactError::MalformedEnvelope(env_name.to_string(), e.to_string()))?;

    // Decrypt and authenticate.
    let plaintext = crate::crypto::decrypt(&key, &ct_bytes, &nonce_bytes).map_err(|_| {
        ArtifactError::MalformedEnvelope(env_name.to_string(), "authentication failed".to_string())
    })?;

    // Deserialize the inner JSON.
    let raw: BTreeMap<String, String> = serde_json::from_slice(&plaintext)
        .map_err(|e| ArtifactError::MalformedEnvelope(env_name.to_string(), e.to_string()))?;

    let secrets = raw
        .into_iter()
        .map(|(k, v)| (k, Zeroizing::new(v)))
        .collect();

    Ok(ArtifactPayload { secrets })
}

// ---------------------------------------------------------------------------
// Tests (T005–T010)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the default KDF params for tests.
    /// Uses reduced cost (memory 256 KiB, time 1) so tests run in < 1 second.
    fn test_kdf_params() -> KdfParams {
        KdfParams {
            algorithm: "argon2id".to_string(),
            memory_kib: 256,
            time_cost: 1,
            parallelism: 1,
            salt: Base64::encode_string(&[0u8; 16]),
        }
    }

    /// Builds an ArtifactPayload with one known key-value pair.
    fn one_secret_payload() -> ArtifactPayload {
        let mut secrets = BTreeMap::new();
        secrets.insert(
            "STRIPE_KEY".to_string(),
            Zeroizing::new("sk_test_12345".to_string()),
        );
        ArtifactPayload { secrets }
    }

    // T005 — derive_key_round_trip
    #[test]
    fn derive_key_round_trip() {
        let passphrase = "hunter2";
        let salt: [u8; 16] = [42u8; 16];
        let params = test_kdf_params();
        let key1 = derive_key(passphrase, &salt, &params).expect("derive_key must succeed");
        let key2 = derive_key(passphrase, &salt, &params).expect("derive_key must succeed");
        assert_eq!(
            *key1, *key2,
            "same passphrase + salt must yield the same key"
        );
    }

    // T006 — derive_key_different_salts_produce_different_keys
    #[test]
    fn derive_key_different_salts_produce_different_keys() {
        let passphrase = "hunter2";
        let salt_a: [u8; 16] = [1u8; 16];
        let salt_b: [u8; 16] = [2u8; 16];
        let params = test_kdf_params();
        let key_a = derive_key(passphrase, &salt_a, &params).expect("derive_key must succeed");
        let key_b = derive_key(passphrase, &salt_b, &params).expect("derive_key must succeed");
        assert_ne!(*key_a, *key_b, "different salts must yield different keys");
    }

    // T007 — seal_unseal_envelope_round_trip
    #[test]
    fn seal_unseal_envelope_round_trip() {
        let passphrase = "correct-horse-battery-staple";
        let payload = one_secret_payload();
        let envelope = seal_envelope(passphrase, &payload).expect("seal must succeed");
        let recovered = unseal_envelope(passphrase, "development", &envelope)
            .expect("unseal must succeed with correct passphrase");
        assert_eq!(
            recovered.secrets.get("STRIPE_KEY").map(|v| v.as_str()),
            Some("sk_test_12345"),
            "recovered value must match original"
        );
    }

    // T008 — wrong_passphrase_returns_malformed_envelope
    #[test]
    fn wrong_passphrase_returns_malformed_envelope() {
        let payload = one_secret_payload();
        let envelope = seal_envelope("correct", &payload).expect("seal must succeed");
        let result = unseal_envelope("wrong", "development", &envelope);
        assert!(
            matches!(result, Err(ArtifactError::MalformedEnvelope(_, _))),
            "wrong passphrase must return MalformedEnvelope, got: {:?}",
            result.err()
        );
    }

    // T009 — tampered_ciphertext_returns_malformed_envelope
    #[test]
    fn tampered_ciphertext_returns_malformed_envelope() {
        let payload = one_secret_payload();
        let mut envelope = seal_envelope("passphrase", &payload).expect("seal must succeed");

        // Decode, flip first bit, re-encode.
        let mut ct_bytes =
            Base64::decode_vec(&envelope.ciphertext).expect("ciphertext must be valid Base64");
        ct_bytes[0] ^= 0xFF;
        envelope.ciphertext = Base64::encode_string(&ct_bytes);

        let result = unseal_envelope("passphrase", "development", &envelope);
        assert!(
            matches!(result, Err(ArtifactError::MalformedEnvelope(_, _))),
            "tampered ciphertext must return MalformedEnvelope, got: {:?}",
            result.err()
        );
    }

    // T010 — empty_passphrase_returns_weak_passphrase
    #[test]
    fn empty_passphrase_returns_weak_passphrase() {
        let payload = one_secret_payload();

        let result_empty = seal_envelope("", &payload);
        assert!(
            matches!(result_empty, Err(ArtifactError::WeakPassphrase)),
            "empty passphrase must return WeakPassphrase"
        );

        let result_whitespace = seal_envelope("   ", &payload);
        assert!(
            matches!(result_whitespace, Err(ArtifactError::WeakPassphrase)),
            "whitespace passphrase must return WeakPassphrase"
        );
    }
}
