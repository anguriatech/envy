//! AES-256-GCM stateless encrypt/decrypt.
//!
//! Each [`encrypt`] call generates a fresh nonce from the OS CSPRNG, making nonce reuse
//! structurally impossible. [`decrypt`] verifies the GCM authentication tag before
//! returning any bytes, and returns the plaintext wrapped in [`zeroize::Zeroizing`] so
//! the backing memory is zeroed when the caller drops it.

use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use zeroize::Zeroizing;

use crate::crypto::CryptoError;

// ---------------------------------------------------------------------------
// T013 — EncryptedSecret struct
// ---------------------------------------------------------------------------

/// The output of a single [`encrypt`] call.
///
/// Both fields must be persisted to enable future decryption. They map directly to the
/// `value_encrypted` and `value_nonce` columns in the `secrets` database table.
pub struct EncryptedSecret {
    /// AES-256-GCM ciphertext with the 16-byte GCM authentication tag appended.
    /// Stored in `secrets.value_encrypted`.
    pub ciphertext: Vec<u8>,

    /// The 12-byte (96-bit) random nonce used for this encryption call.
    /// Stored in `secrets.value_nonce`. Must never be reused with the same key.
    pub nonce: [u8; 12],
}

// ---------------------------------------------------------------------------
// T014 — encrypt
// ---------------------------------------------------------------------------

/// Encrypts `plaintext` using AES-256-GCM with a freshly generated random nonce.
///
/// Every call generates a unique nonce from the OS CSPRNG (`OsRng`), so calling this
/// twice with the same input produces different `ciphertext` and `nonce` values.
///
/// # Errors
/// Returns [`CryptoError::EncryptionFailed`] on internal cipher failure. This is
/// structurally impossible with a valid 32-byte key and is treated as an internal error.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedSecret, CryptoError> {
    // SAFETY: key is typed &[u8; 32], so new_from_slice will never return InvalidLength.
    let cipher =
        Aes256Gcm::new_from_slice(key).expect("key is &[u8; 32]: length is always valid");
    let nonce_ga = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce_ga, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;
    // SAFETY: Aes256Gcm::generate_nonce returns exactly U12 (12) bytes by construction.
    let nonce: [u8; 12] = nonce_ga
        .as_slice()
        .try_into()
        .expect("generate_nonce always returns exactly 12 bytes");
    Ok(EncryptedSecret { ciphertext, nonce })
}

// ---------------------------------------------------------------------------
// T015 — decrypt
// ---------------------------------------------------------------------------

/// Decrypts `ciphertext` and verifies the GCM authentication tag.
///
/// Returns the original plaintext wrapped in [`Zeroizing`], which zeroes the backing
/// memory when the value is dropped (Constitution Principle I).
///
/// # Errors
/// - [`CryptoError::InvalidNonce`] if `nonce` is not exactly 12 bytes.
/// - [`CryptoError::DecryptionFailed`] if the authentication tag does not verify (wrong
///   key, tampered ciphertext, or wrong nonce). No partial plaintext is ever exposed.
pub fn decrypt(
    key: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if nonce.len() != 12 {
        return Err(CryptoError::InvalidNonce);
    }
    // SAFETY: key is typed &[u8; 32], so new_from_slice will never return InvalidLength.
    let cipher =
        Aes256Gcm::new_from_slice(key).expect("key is &[u8; 32]: length is always valid");
    // GenericArray::from_slice would panic if len != 12; we validated above so this is safe.
    // Type inference resolves the size from cipher.decrypt's expected &Nonce<Aes256Gcm>.
    let nonce = GenericArray::from_slice(nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)?;
    Ok(Zeroizing::new(plaintext))
}

// ---------------------------------------------------------------------------
// T005–T012 — Tests (written to define the contract; must fail before T013–T015)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: [u8; 32] = [0u8; 32];
    const KEY_B: [u8; 32] = [1u8; 32];

    // T005
    #[test]
    fn encrypt_produces_ciphertext() {
        let result = encrypt(&KEY_A, b"hello").expect("encrypt must succeed");
        assert_ne!(
            result.ciphertext,
            b"hello".to_vec(),
            "ciphertext must differ from plaintext"
        );
        assert!(
            !result.ciphertext.windows(5).any(|w| w == b"hello"),
            "plaintext must not appear as a substring of ciphertext"
        );
        assert_eq!(result.nonce.len(), 12, "nonce must be exactly 12 bytes");
    }

    // T006
    #[test]
    fn decrypt_round_trips() {
        let secret = encrypt(&KEY_A, b"hello").expect("encrypt must succeed");
        let plaintext = decrypt(&KEY_A, &secret.ciphertext, &secret.nonce)
            .expect("decrypt must succeed with matching key");
        assert_eq!(plaintext.as_slice(), b"hello");
    }

    // T007
    #[test]
    fn wrong_key_fails() {
        let secret = encrypt(&KEY_A, b"hello").expect("encrypt must succeed");
        let result = decrypt(&KEY_B, &secret.ciphertext, &secret.nonce);
        assert!(
            matches!(result, Err(CryptoError::DecryptionFailed)),
            "wrong key must return DecryptionFailed, got: {:?}",
            result
        );
    }

    // T008
    #[test]
    fn tampered_ciphertext_fails() {
        let mut secret = encrypt(&KEY_A, b"hello").expect("encrypt must succeed");
        secret.ciphertext[0] ^= 0xFF; // flip all bits in the first byte
        let result = decrypt(&KEY_A, &secret.ciphertext, &secret.nonce);
        assert!(
            matches!(result, Err(CryptoError::DecryptionFailed)),
            "tampered ciphertext must return DecryptionFailed, got: {:?}",
            result
        );
    }

    // T009
    #[test]
    fn empty_plaintext_succeeds() {
        let secret =
            encrypt(&KEY_A, b"").expect("encrypt must succeed for empty plaintext");
        let plaintext = decrypt(&KEY_A, &secret.ciphertext, &secret.nonce)
            .expect("decrypt must succeed for empty plaintext");
        assert_eq!(plaintext.as_slice(), b"");
    }

    // T010
    #[test]
    fn nonce_uniqueness() {
        let s1 = encrypt(&KEY_A, b"hello").expect("first encrypt must succeed");
        let s2 = encrypt(&KEY_A, b"hello").expect("second encrypt must succeed");
        assert_ne!(
            s1.nonce, s2.nonce,
            "each encrypt call must generate a unique nonce"
        );
    }

    // T011
    #[test]
    fn invalid_nonce_length() {
        let secret = encrypt(&KEY_A, b"hello").expect("encrypt must succeed");
        let result_short = decrypt(&KEY_A, &secret.ciphertext, &[0u8; 11]);
        assert!(
            matches!(result_short, Err(CryptoError::InvalidNonce)),
            "11-byte nonce must return InvalidNonce, got: {:?}",
            result_short
        );
        let result_long = decrypt(&KEY_A, &secret.ciphertext, &[0u8; 13]);
        assert!(
            matches!(result_long, Err(CryptoError::InvalidNonce)),
            "13-byte nonce must return InvalidNonce, got: {:?}",
            result_long
        );
    }

    // T012
    #[test]
    fn zeroize_plaintext() {
        use std::mem::ManuallyDrop;
        use zeroize::Zeroize;

        let secret = encrypt(&KEY_A, b"secret data").expect("encrypt must succeed");
        let mut decrypted = ManuallyDrop::new(
            decrypt(&KEY_A, &secret.ciphertext, &secret.nonce)
                .expect("decrypt must succeed"),
        );

        let ptr = decrypted.as_ptr();
        let original_len = decrypted.len();

        // Call zeroize explicitly — identical to what Zeroizing's Drop impl does.
        // This zeroes ptr..ptr+original_len then calls Vec::clear() (len→0, buffer kept).
        (*decrypted).zeroize();

        // SAFETY: ManuallyDrop keeps the Vec's buffer alive — memory is still valid here.
        // We verify while the allocation is live because glibc's tcache overwrites freed
        // chunks on Linux, making post-free reads unreliable even when zeroize is correct.
        let zeroed = unsafe { std::slice::from_raw_parts(ptr, original_len) };
        assert!(
            zeroed.iter().all(|&b| b == 0),
            "plaintext bytes must be zeroed after zeroize"
        );

        // SAFETY: We own the allocation via ManuallyDrop; this frees it.
        unsafe { ManuallyDrop::drop(&mut decrypted) };
    }
}
