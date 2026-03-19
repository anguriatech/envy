# Tasks: Crypto Layer

**Feature**: 002-crypto-layer
**Input**: spec.md, plan.md, contracts/crypto-layer.md
**Total tasks**: 26

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[US?]**: Which user story this task belongs to
- Every test from `plan.md` has its own explicit task
- Tests are written **before** their corresponding implementation (TDD)

---

## Phase 1: Setup

**Purpose**: Add new dependencies and expand the `src/crypto/` scaffold.

- [x] T001 Add `aes-gcm = "0.10"` and `zeroize = { version = "1", features = ["derive"] }` to `[dependencies]` in `Cargo.toml`
- [x] T002 Replace the stub comment in `src/crypto/mod.rs` with empty `mod` declarations for `error`, `aead`, and `keyring` (no `pub use` yet — those come per-phase); run `cargo build` to confirm the project still compiles

**Checkpoint**: `cargo build` succeeds with the two new crates resolved.

---

## Phase 2: CryptoError Enum

**Purpose**: Define the typed error surface that all subsequent modules depend on. This phase must be complete before writing any test or implementation in phases 3 and 4.

- [x] T003 Create `src/crypto/error.rs` with the full `CryptoError` enum (6 variants: `KeyNotFound`, `KeyCorrupted`, `KeyringUnavailable(String)`, `EncryptionFailed`, `DecryptionFailed`, `InvalidNonce`) using `#[derive(Debug, thiserror::Error)]` with the display strings from `contracts/crypto-layer.md`
- [x] T004 Update `src/crypto/mod.rs` to add `mod error;` and `pub use error::CryptoError;`; run `cargo build` to confirm `CryptoError` is accessible as `envy::crypto::CryptoError`

**Checkpoint**: `cargo build` succeeds; `envy::crypto::CryptoError` is a public type.

---

## Phase 3: AEAD Module — `aead.rs` (US2 + US3)

**Goal**: Implement stateless AES-256-GCM encrypt/decrypt with per-call random nonces and
zeroed plaintext output. Covers User Story 2 (encrypt/decrypt round-trip) and User Story 3
(memory zeroing).

**Independent Test**: `cargo test` passes all 8 tests in `src/crypto/aead.rs`.

### Tests — write first, verify they FAIL before implementation (T005–T012)

- [x] T005 [US2] Write test `encrypt_produces_ciphertext` in `src/crypto/aead.rs` (`#[cfg(test)]`): call `encrypt` with a 32-byte all-zero key and plaintext `b"hello"`, assert `result.ciphertext` does not contain the bytes of `b"hello"` and `result.nonce` is exactly 12 bytes
- [x] T006 [US2] Write test `decrypt_round_trips` in `src/crypto/aead.rs`: call `encrypt` then `decrypt` with the same key; assert the decrypted output equals `b"hello"` byte-for-byte
- [x] T007 [US2] Write test `wrong_key_fails` in `src/crypto/aead.rs`: encrypt with key A, then call `decrypt` with key B (all-ones); assert `Err(CryptoError::DecryptionFailed)`
- [x] T008 [US2] Write test `tampered_ciphertext_fails` in `src/crypto/aead.rs`: encrypt, flip one byte in the ciphertext, call `decrypt`; assert `Err(CryptoError::DecryptionFailed)`
- [x] T009 [US2] Write test `empty_plaintext_succeeds` in `src/crypto/aead.rs`: call `encrypt` with an empty slice `b""`; assert `Ok(_)` and that `decrypt` round-trips back to `b""`
- [x] T010 [US2] Write test `nonce_uniqueness` in `src/crypto/aead.rs`: call `encrypt` twice with the same plaintext and key; assert the two returned `nonce` values are different
- [x] T011 [US2] Write test `invalid_nonce_length` in `src/crypto/aead.rs`: call `decrypt` with a nonce of 11 bytes and again with 13 bytes; assert both return `Err(CryptoError::InvalidNonce)`
- [x] T012 [US3] Write test `zeroize_plaintext` in `src/crypto/aead.rs`: uses ManuallyDrop to keep buffer alive, calls zeroize() explicitly (same as Drop impl), reads bytes via raw pointer while still allocated, asserts all zero

### Implementation (T013–T016)

- [x] T013 [US2] Define the `EncryptedSecret` struct in `src/crypto/aead.rs` with fields `ciphertext: Vec<u8>` and `nonce: [u8; 12]` (matching `contracts/crypto-layer.md`)
- [x] T014 [US2] Implement `pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedSecret, CryptoError>` in `src/crypto/aead.rs`: build `Aes256Gcm` from key, generate a fresh `[u8; 12]` nonce from `OsRng`, call `cipher.encrypt`, map errors to `CryptoError::EncryptionFailed`
- [x] T015 [US2] Implement `pub fn decrypt(key: &[u8; 32], ciphertext: &[u8], nonce: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError>` in `src/crypto/aead.rs`: validate `nonce.len() == 12` → `InvalidNonce`, build cipher, call `cipher.decrypt`, map tag-mismatch/any-error to `DecryptionFailed`, wrap result in `Zeroizing`
- [x] T016 Update `src/crypto/mod.rs` to add `mod aead;` and `pub use aead::{encrypt, decrypt, EncryptedSecret};`; run `cargo test` — all 8 aead tests must pass

**Checkpoint**: `cargo test` — 8 new aead tests pass; all 38 tests from feature 001 still pass.

---

## Phase 4: Keyring Module — `keyring.rs` (US1)

**Goal**: Implement master key fetch-or-generate via the OS Credential Manager. Covers User
Story 1 (vault unlocks on first run).

**Independent Test**: `cargo test` — `key_length_validated` passes; `get_or_create_master_key`
passes when a Secret Service daemon is available (or is skipped via `#[ignore]`).

### Tests — write first, verify they FAIL before implementation (T017–T018)

- [x] T017 [US1] Write test `get_or_create_master_key_is_idempotent` (marked `#[ignore]`) in `src/crypto/keyring.rs`: calls twice, asserts 32 bytes and idempotency; skipped without a live daemon
- [x] T018 [US1] Write test `key_length_validated` in `src/crypto/keyring.rs`: calls `decode_key` (the testable validation helper) with short/long/valid hex strings; no live keyring required

### Implementation (T019–T021)

- [x] T019 [US1] Define `SERVICE_NAME`/`ACCOUNT_NAME` constants; `encode_key`/`decode_key`/`hex_nibble` hex helpers; `retrieve_key` private helper mapping `NoEntry`→`KeyNotFound`, corrupt hex→`KeyCorrupted`, other errors→`KeyringUnavailable`
- [x] T020 [US1] Implement `get_or_create_master_key()`: on `KeyNotFound` generates 32 bytes via `Aes256Gcm::generate_key(OsRng)`, stores as hex via `set_password`; returns `Zeroizing<[u8; 32]>`
- [x] T021 Update `src/crypto/mod.rs` to add `mod keyring;` and `pub use keyring::get_or_create_master_key;`

**Checkpoint**: `cargo test` — `key_length_validated` passes; full suite (38 + 8 aead + keyring) green.

---

## Phase 5: Polish

**Purpose**: Quality gates, linting, audit, and documentation update.

- [x] T022 [P] Run `cargo clippy -- -D warnings` and fix ALL warnings in `src/crypto/`
- [x] T023 [P] Run the full test suite `cargo test` — all tests (feature 001 + crypto) must pass with 0 failures
- [x] T024 [P] Run `cargo audit` — 0 new vulnerabilities introduced by `aes-gcm` or `zeroize`
- [x] T025 Update `CLAUDE.md` active technologies line to include `aes-gcm` and `zeroize` (with the `derive` feature) alongside the existing crates

**Checkpoint**: All 4 polish tasks pass. Feature 002 is complete.

---

## Dependencies & Execution Order

```
T001 → T002 → T003 → T004   (setup + error — strictly sequential)
                    ↓
         T005–T012           (aead tests — write all, then...)
                    ↓
         T013 → T014 → T015 → T016   (aead implementation — sequential within file)
                                   ↓
                        T017–T018   (keyring tests — then...)
                                   ↓
                        T019 → T020 → T021   (keyring implementation)
                                           ↓
                              T022 [P] T023 [P] T024 [P]   (polish — parallel)
                                           ↓
                                          T025
```

### Phase dependencies

- **Phase 1** (T001–T002): No dependencies — start immediately
- **Phase 2** (T003–T004): Requires Phase 1 — `CryptoError` is used in aead + keyring tests
- **Phase 3** (T005–T016): Requires Phase 2 — tests import `CryptoError`
- **Phase 4** (T017–T021): Requires Phase 2 — tests import `CryptoError`; can start in parallel with Phase 3 if needed (different file), but sequential is safer
- **Phase 5** (T022–T025): Requires Phases 3 and 4 complete

---

## Implementation Strategy

### Single-developer sequence (recommended)

1. Phase 1 → 2 → 3 (tests then implementation) → 4 (tests then implementation) → 5
2. Run `cargo test` after T016 and after T021 as interim checkpoints
3. Phase 5 is the final gate before marking the feature complete
