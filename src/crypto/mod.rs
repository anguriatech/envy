//! Cryptography layer — encryption, decryption, and OS credential management.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::db`.
//! - MUST NOT know about database schemas.
//!
//! # Public API
//! - [`CryptoError`] — typed error enum for all crypto operations.
//! - `encrypt` / `decrypt` / [`EncryptedSecret`] — AES-256-GCM AEAD (Phase 3).
//! - `get_or_create_master_key` — OS Credential Manager key management (Phase 4).

mod error;
mod aead;
mod keyring;

pub use error::CryptoError;
pub use aead::{decrypt, encrypt, EncryptedSecret};
pub use keyring::get_or_create_master_key;
