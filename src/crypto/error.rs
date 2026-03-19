//! Typed error enum for all cryptography layer operations.

/// All errors that can be returned by the cryptography layer.
///
/// # Design notes
/// - [`DecryptionFailed`](CryptoError::DecryptionFailed) is deliberately opaque: it does
///   not reveal whether the failure was a tag mismatch or a wrong key, which would aid
///   oracle attacks.
/// - [`KeyringUnavailable`](CryptoError::KeyringUnavailable) carries a diagnostic string
///   for operator troubleshooting. That string MUST NOT contain key bytes.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// No master key entry exists in the OS credential store.
    #[error("no master key found in the OS credential store")]
    KeyNotFound,

    /// A key entry exists in the OS credential store but its byte length is not 32.
    #[error("master key in credential store has invalid length (expected 32 bytes)")]
    KeyCorrupted,

    /// The OS credential manager could not be reached or returned an unexpected error.
    ///
    /// The inner string is a diagnostic message for troubleshooting.
    /// It MUST NOT contain key bytes or secret values.
    #[error("OS credential manager is unavailable: {0}")]
    KeyringUnavailable(String),

    /// AES-256-GCM encryption failed.
    ///
    /// Should be structurally impossible with valid inputs; treat as an internal error.
    #[error("encryption failed")]
    EncryptionFailed,

    /// AES-256-GCM decryption or tag verification failed.
    ///
    /// Returned for any of: wrong key, tampered ciphertext, wrong nonce.
    /// Deliberately opaque to prevent oracle attacks.
    #[error("decryption failed: ciphertext is invalid or key is wrong")]
    DecryptionFailed,

    /// The nonce slice passed to `decrypt` was not exactly 12 bytes.
    #[error("nonce must be exactly 12 bytes")]
    InvalidNonce,
}
