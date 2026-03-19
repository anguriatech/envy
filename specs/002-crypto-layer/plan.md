# Implementation Plan: 002-crypto-layer

**Feature**: Crypto Layer
**Branch**: `002-crypto-layer`
**Date**: 2026-03-19
**Status**: Awaiting approval

---

## 1. Scope

Implement `src/crypto/` as a self-contained Rust module with two responsibilities:

1. **Master Key Management** — fetch-or-generate the vault master key via the OS
   Credential Manager (`keyring` crate).
2. **AEAD Encryption/Decryption** — encrypt and decrypt secret values using AES-256-GCM
   (`aes-gcm` crate from RustCrypto), producing the `value_encrypted` and `value_nonce`
   column values required by the DB schema from Feature 001.

This layer has no knowledge of the database schema, CLI arguments, or business rules.

---

## 2. Architecture Position

```
src/
├── cli/        ← (stub) UI layer — not touched
├── core/       ← (stub) Business logic — caller of this layer
├── crypto/     ← THIS FEATURE
│   ├── mod.rs          pub re-exports; module doc
│   ├── error.rs        CryptoError enum
│   ├── keyring.rs      Master key: fetch / store / generate
│   └── aead.rs         encrypt / decrypt (AES-256-GCM)
└── db/         ← (complete) Database layer — not touched
```

**Dependency rule** (Constitution Principle IV):
```
core → crypto   ✓  (only direction allowed)
crypto → db     ✗  (prohibited)
crypto → cli    ✗  (prohibited)
```

---

## 3. New Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
aes-gcm  = "0.10"      # AES-256-GCM AEAD (RustCrypto)
zeroize  = { version = "1", features = ["derive"] }  # Memory zeroing

[dev-dependencies]
# no new dev deps — tempfile already present from feature 001
```

> `keyring = "2"` and `thiserror = "1"` are already present from Feature 001.

---

## 4. Module Design

### 4.1 `error.rs` — `CryptoError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("no master key found in the OS credential store")]
    KeyNotFound,

    #[error("master key in credential store has invalid length (expected 32 bytes)")]
    KeyCorrupted,

    #[error("OS credential manager is unavailable: {0}")]
    KeyringUnavailable(String),

    #[error("encryption failed")]
    EncryptionFailed,

    #[error("decryption failed: ciphertext is invalid or key is wrong")]
    DecryptionFailed,

    #[error("nonce must be exactly 12 bytes")]
    InvalidNonce,
}
```

**Design notes**:
- `KeyringUnavailable` carries the underlying error string for diagnostics — but MUST NOT
  expose key bytes.
- `DecryptionFailed` deliberately omits whether the failure was a tag mismatch or a wrong
  key; leaking that distinction aids oracle attacks.

---

### 4.2 `keyring.rs` — Master Key Management

**Constants** (private to this module):
```
SERVICE_NAME = "envy"
ACCOUNT_NAME = "master-key"
```

**Public API** (all `pub(super)` — re-exported from `mod.rs` as `pub`):

```
get_or_create_master_key() -> Result<Zeroizing<[u8; 32]>, CryptoError>
```

Behaviour:
1. Open the OS keyring entry for `(SERVICE_NAME, ACCOUNT_NAME)`.
2. If the entry exists: decode the stored bytes, validate length == 32, return
   `Zeroizing<[u8; 32]>`. If length is wrong → `KeyCorrupted`.
3. If the entry does not exist (`keyring::Error::NoEntry`): generate 32 bytes from
   `OsRng`, store them in the keyring (hex-encoded or raw bytes — TBD at implementation),
   return the key wrapped in `Zeroizing`.
4. If the keyring is unavailable for any other reason → `KeyringUnavailable(msg)`.

**Storage format**: the key is stored as raw bytes via the keyring crate's `set_password`
/ `get_password` API using a `Secret<Vec<u8>>` (if the crate supports binary secrets) or
as a hex-encoded string otherwise. The exact wire format is an implementation detail.

---

### 4.3 `aead.rs` — AES-256-GCM Encryption / Decryption

**Public API** (re-exported from `mod.rs`):

```
encrypt(
    key:       &[u8; 32],
    plaintext: &[u8],
) -> Result<EncryptedSecret, CryptoError>

decrypt(
    key:        &[u8; 32],
    ciphertext: &[u8],
    nonce:      &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError>
```

**`EncryptedSecret`** (return type of `encrypt`):
```rust
pub struct EncryptedSecret {
    pub ciphertext: Vec<u8>,   // → value_encrypted column
    pub nonce:      [u8; 12],  // → value_nonce column
}
```

`encrypt` behaviour:
1. Build an `Aes256Gcm` cipher from the 32-byte key.
2. Generate a fresh 12-byte nonce from `OsRng` for every call.
3. Encrypt with `cipher.encrypt(nonce, plaintext)` — this appends the 16-byte GCM tag
   to the ciphertext automatically.
4. Return `EncryptedSecret { ciphertext, nonce }`.

`decrypt` behaviour:
1. Validate `nonce.len() == 12` → `InvalidNonce` if not.
2. Build an `Aes256Gcm` cipher from the key.
3. Decrypt with `cipher.decrypt(nonce, ciphertext)` — verifies GCM tag internally.
4. On tag mismatch or any decryption error → `DecryptionFailed`.
5. Return `Zeroizing<Vec<u8>>` wrapping the plaintext.

**Why `Zeroizing` on the return value**: the Core layer will immediately use the plaintext
to inject it into a child process environment or display it; it must be zeroed as soon as
the caller drops it (Constitution Principle I).

---

### 4.4 `mod.rs` — Public Re-exports

```rust
mod error;
mod keyring;
mod aead;

pub use error::CryptoError;
pub use keyring::get_or_create_master_key;
pub use aead::{encrypt, decrypt, EncryptedSecret};
```

No business logic lives in `mod.rs`.

---

## 5. Memory Safety Plan

| Value | Held in | Zeroed by |
|-------|---------|-----------|
| Master key (32 bytes) | `Zeroizing<[u8; 32]>` | `Zeroizing` drop impl |
| Decrypted plaintext | `Zeroizing<Vec<u8>>` | `Zeroizing` drop impl |
| Intermediate key schedule | Inside `Aes256Gcm` (stack) | Dropped at end of `encrypt`/`decrypt` call |
| Nonce | `[u8; 12]` — not sensitive | N/A |
| Ciphertext | `Vec<u8>` — not sensitive | N/A |

---

## 6. Test Plan

All tests live in `src/crypto/` as Rust unit tests (`#[cfg(test)]` blocks), since this
layer has no DB or CLI dependencies — no `tempfile` or integration test harness needed.

| Test | Location | What it verifies |
|------|----------|-----------------|
| `encrypt_produces_ciphertext` | `aead.rs` | Ciphertext != plaintext |
| `decrypt_round_trips` | `aead.rs` | `decrypt(encrypt(p)) == p` |
| `wrong_key_fails` | `aead.rs` | `DecryptionFailed` on wrong key |
| `tampered_ciphertext_fails` | `aead.rs` | `DecryptionFailed` on bit flip |
| `empty_plaintext_succeeds` | `aead.rs` | Empty input is valid |
| `nonce_uniqueness` | `aead.rs` | Two encryptions of same plaintext yield different nonces |
| `invalid_nonce_length` | `aead.rs` | `InvalidNonce` for non-12-byte nonce |
| `zeroize_plaintext` | `aead.rs` | Backing bytes are zero after drop |
| `get_or_create_master_key` | `keyring.rs` | Integration test — requires live keyring |
| `key_length_validated` | `keyring.rs` | `KeyCorrupted` if stored value is wrong length |

> The `get_or_create_master_key` test is gated with `#[cfg(feature = "keyring-tests")]`
> or marked `#[ignore]` so CI without a Secret Service daemon does not fail.

---

## 7. Constitution Compliance

| Principle | How this feature complies |
|-----------|--------------------------|
| I. Security | Master key never written to disk; plaintext zeroed on drop; errors never expose key bytes |
| II. Determinism | `OsRng` is a CSPRNG documented as such; nonce generation is explicit |
| III. Rust Best Practices | `thiserror` typed errors; no `.unwrap()`; full unit test coverage |
| IV. Modularity | `crypto/` imports nothing from `cli/`, `core/`, or `db/` |
| V. Language | All identifiers, comments, and docs in English |

---

## 8. Out of Scope

- Key derivation (KDF / Argon2) — the master key is randomly generated, not derived from
  a passphrase. KDF is a future amendment if passphrase-protected vaults are added.
- Key rotation — designing the rotation flow is deferred to a future feature.
- Authenticated encryption of the vault file itself — handled by SQLCipher (Feature 001).
- Any CLI commands or user-facing output — belongs to the UI layer.
