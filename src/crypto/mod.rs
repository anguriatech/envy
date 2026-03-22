//! Cryptography layer — encryption, decryption, and OS credential management.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::db`.
//! - MUST NOT know about database schemas.
//!
//! # Public API
//! - [`CryptoError`] — typed error enum for all crypto operations.
//! - `encrypt` / `decrypt` / [`EncryptedSecret`] — AES-256-GCM AEAD.
//! - `get_or_create_master_key` — OS Credential Manager key management.
//! - [`artifact`] — GitOps sync artifact cryptography (`envy.enc`).

mod aead;
pub mod artifact;
mod error;
mod keyring;

pub use aead::{EncryptedSecret, decrypt, encrypt};
pub use artifact::{
    ARTIFACT_VERSION, ArtifactError, ArtifactPayload, EncryptedEnvelope, KDF_MEMORY_KIB,
    KDF_PARALLELISM, KDF_SALT_BYTES, KDF_TIME_COST, KdfParams, SyncArtifact, derive_key,
    seal_envelope, unseal_envelope,
};
pub use error::CryptoError;
pub use keyring::get_or_create_master_key;
