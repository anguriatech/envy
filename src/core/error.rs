//! Typed error surface for the Core Logic layer.

use crate::crypto::CryptoError;
use crate::db::DbError;

/// All errors that the Core Logic layer can return to the CLI layer.
///
/// `#[from]` on `Db` and `Crypto` generates `From<DbError>` and
/// `From<CryptoError>` implementations, enabling `?`-based propagation
/// from `ops.rs` functions without explicit `map_err` calls.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Db(#[from] DbError),

    /// A cryptographic operation failed.
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),

    /// No `envy.toml` was found in the current directory or any parent.
    #[error("not an envy project (run `envy init` to initialize)")]
    ManifestNotFound,

    /// `envy.toml` was found but could not be parsed or is missing required fields.
    #[error("envy.toml is invalid: {0}")]
    ManifestInvalid(String),

    /// An I/O error occurred while reading or writing `envy.toml`.
    #[error("could not read/write envy.toml: {0}")]
    ManifestIo(String),

    /// The secret key name failed validation (empty or contains `=`).
    #[error("invalid secret key name \"{0}\": must be non-empty and must not contain `=`")]
    InvalidSecretKey(String),
}
