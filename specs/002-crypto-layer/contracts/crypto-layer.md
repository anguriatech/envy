# Contract: Crypto Layer Public API

**Feature**: 002-crypto-layer
**Date**: 2026-03-19
**Stability**: Draft — awaiting approval

This document defines the complete public API surface of `src/crypto/`. The Core layer
(`src/core/`) is the only permitted caller. No other layer may import from `src/crypto/`
directly.

---

## Error Type

```rust
// src/crypto/error.rs — re-exported as envy::crypto::CryptoError
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// No master key entry exists in the OS credential store.
    #[error("no master key found in the OS credential store")]
    KeyNotFound,

    /// A key entry exists but its byte length is not 32.
    #[error("master key in credential store has invalid length (expected 32 bytes)")]
    KeyCorrupted,

    /// The OS credential manager could not be reached or returned an unexpected error.
    /// The inner string is a diagnostic message; it MUST NOT contain key bytes.
    #[error("OS credential manager is unavailable: {0}")]
    KeyringUnavailable(String),

    /// AES-256-GCM encryption failed (should be structurally impossible with valid inputs).
    #[error("encryption failed")]
    EncryptionFailed,

    /// AES-256-GCM decryption or tag verification failed.
    /// Returned for wrong key, tampered ciphertext, or any authentication failure.
    /// Deliberately opaque to prevent oracle attacks.
    #[error("decryption failed: ciphertext is invalid or key is wrong")]
    DecryptionFailed,

    /// The nonce slice passed to `decrypt` was not exactly 12 bytes.
    #[error("nonce must be exactly 12 bytes")]
    InvalidNonce,
}
```

---

## Return Type

```rust
// src/crypto/aead.rs — re-exported as envy::crypto::EncryptedSecret
/// The output of a single `encrypt` call.
/// Both fields must be stored (e.g., in the DB schema's value_encrypted / value_nonce
/// columns) to enable future decryption.
pub struct EncryptedSecret {
    /// The AES-256-GCM ciphertext (includes the 16-byte GCM authentication tag appended
    /// by the aes-gcm crate). Stored in `secrets.value_encrypted`.
    pub ciphertext: Vec<u8>,

    /// The 12-byte (96-bit) random nonce used for this encryption.
    /// Stored in `secrets.value_nonce`. Must never be reused with the same key.
    pub nonce: [u8; 12],
}
```

---

## Functions

### `get_or_create_master_key`

```rust
pub fn get_or_create_master_key() -> Result<Zeroizing<[u8; 32]>, CryptoError>
```

**Purpose**: Returns the 32-byte vault master key. If no key exists in the OS Credential
Manager, a new cryptographically random key is generated and stored first.

**Caller contract**:
- Call this once per command invocation to obtain the key, then pass it to `encrypt` /
  `decrypt`.
- The returned `Zeroizing<[u8; 32]>` MUST be dropped as soon as the key is no longer
  needed; do not clone or persist it.

**Guarantees**:
- On `Ok`: the returned slice is exactly 32 bytes.
- On `Err(KeyNotFound)`: the key was not found and could not be created (should not occur
  under normal conditions — auto-creation is attempted first).
- On `Err(KeyCorrupted)`: a key entry exists but is not 32 bytes; the vault is unusable
  until the entry is deleted and re-created.
- On `Err(KeyringUnavailable)`: the OS Credential Manager daemon is not running or access
  was denied.

**Side effects**: may write a new entry to the OS Credential Manager on first call.

---

### `encrypt`

```rust
pub fn encrypt(
    key:       &[u8; 32],
    plaintext: &[u8],
) -> Result<EncryptedSecret, CryptoError>
```

**Purpose**: Encrypts `plaintext` using AES-256-GCM with a freshly generated random nonce.

**Caller contract**:
- `key` MUST be the 32-byte value returned by `get_or_create_master_key`.
- `plaintext` may be empty (valid AEAD input).
- The caller MUST persist both `EncryptedSecret.ciphertext` and `EncryptedSecret.nonce`;
  losing either makes decryption permanently impossible.

**Guarantees**:
- On `Ok`: `nonce` is exactly 12 bytes; `ciphertext` includes the 16-byte GCM tag.
- Every call generates a fresh nonce from `OsRng`; calling `encrypt` twice with the same
  plaintext produces different `ciphertext` and `nonce` values.
- On `Err(EncryptionFailed)`: should not occur with valid inputs; treat as an internal
  error.

**Side effects**: none (reads entropy from OS).

---

### `decrypt`

```rust
pub fn decrypt(
    key:        &[u8; 32],
    ciphertext: &[u8],
    nonce:      &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError>
```

**Purpose**: Decrypts `ciphertext` and verifies its GCM authentication tag. Returns the
original plaintext zeroed on drop.

**Caller contract**:
- `key` MUST be the same 32-byte key used during `encrypt`.
- `nonce` MUST be the `EncryptedSecret.nonce` stored alongside the ciphertext — exactly
  12 bytes.
- `ciphertext` MUST include the 16-byte GCM tag (as stored from `EncryptedSecret.ciphertext`).

**Guarantees**:
- On `Ok`: the returned `Zeroizing<Vec<u8>>` contains the exact original plaintext; its
  backing memory is zeroed when the value is dropped.
- On `Err(InvalidNonce)`: `nonce.len() != 12`; no decryption was attempted.
- On `Err(DecryptionFailed)`: the GCM tag did not verify (wrong key, tampered ciphertext,
  or wrong nonce). No partial plaintext is accessible.

**Side effects**: none.

---

## Invariants

1. **No plaintext on disk**: this layer never writes key material or plaintext to any file,
   database, or log.
2. **No cross-layer imports**: `src/crypto/` MUST NOT import `src/db/`, `src/core/`, or
   `src/cli/`.
3. **Nonce uniqueness**: `encrypt` generates a new nonce per call using `OsRng`; the same
   nonce is never reused with the same key.
4. **Tag verification before plaintext exposure**: `decrypt` returns data only after the
   GCM tag is verified; partial decryption results are never accessible to the caller.
5. **Zeroing on drop**: `get_or_create_master_key` returns `Zeroizing<[u8; 32]>`;
   `decrypt` returns `Zeroizing<Vec<u8>>`; both zero their memory when dropped.

---

## DB Schema Mapping

| Crypto value | DB column | Type |
|---|---|---|
| `EncryptedSecret.ciphertext` | `secrets.value_encrypted` | `BLOB NOT NULL` |
| `EncryptedSecret.nonce` | `secrets.value_nonce` | `BLOB NOT NULL` (12 bytes) |

The Core layer is responsible for reading these columns from `Vault` and passing them to
`decrypt`. The crypto layer has no knowledge of the DB schema.
