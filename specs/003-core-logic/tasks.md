# Tasks: Core Logic

**Feature**: 003-core-logic
**Input**: spec.md, plan.md, contracts/core-logic.md
**Total tasks**: 34

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[US?]**: Which user story this task belongs to
- Every test from `plan.md` has its own explicit task
- Tests are written **before** their corresponding implementation (TDD)

---

## Phase 1: Setup

**Purpose**: Add new dependencies and expand the `src/core/` scaffold.

- [x] T001 Add `toml = "0.8"` and `serde = { version = "1", features = ["derive"] }` to `[dependencies]` in `Cargo.toml`
- [x] T002 Create `src/core/mod.rs` with empty `mod` declarations for `error`, `manifest`, and `ops` (no `pub use` yet — those come per-phase); add `pub mod core;` to `src/lib.rs`; run `cargo build` to confirm the project still compiles

**Checkpoint**: `cargo build` succeeds with the two new crates resolved.

---

## Phase 2: CoreError Enum

**Purpose**: Define the typed error surface that all subsequent modules depend on. This phase must be complete before writing any test or implementation in phases 3 and 4.

- [x] T003 Create `src/core/error.rs` with the full `CoreError` enum (6 variants: `Db(#[from] DbError)`, `Crypto(#[from] CryptoError)`, `ManifestNotFound`, `ManifestInvalid(String)`, `ManifestIo(String)`, `InvalidSecretKey(String)`) using `#[derive(Debug, thiserror::Error)]` with the display strings from `contracts/core-logic.md`
- [x] T004 Update `src/core/mod.rs` to add `mod error;` and `pub use error::CoreError;`; run `cargo build` to confirm `CoreError` is accessible as `envy::core::CoreError`

**Checkpoint**: `cargo build` succeeds; `envy::core::CoreError` is a public type.

---

## Phase 3: Manifest Module — `manifest.rs` (US1)

**Goal**: Implement `envy.toml` discovery (directory-tree walk) and creation. Covers User
Story 1 (project context auto-resolved).

**Independent Test**: `cargo test` passes all 4 tests in `src/core/manifest.rs`.

### Tests — write first, verify they FAIL before implementation (T005–T008)

- [x] T005 [US1] Write test `find_manifest_in_current_dir` in `src/core/manifest.rs` (`#[cfg(test)]`): create a temp directory with `envy.toml` containing a valid `project_id`, call `find_manifest` with that directory, assert the returned `Manifest.project_id` matches and the returned path equals the temp dir
- [x] T006 [US1] Write test `find_manifest_in_parent_dir` in `src/core/manifest.rs`: create a temp directory tree `parent/child/grandchild/`, place `envy.toml` in `parent/`, call `find_manifest` with `grandchild/`, assert the returned manifest matches the one in `parent/` and the returned path equals `parent/`
- [x] T007 [US1] Write test `find_manifest_not_found` in `src/core/manifest.rs`: call `find_manifest` starting from a temp directory that has no `envy.toml` in any ancestor up to the filesystem root; assert `Err(CoreError::ManifestNotFound)`
- [x] T008 [US1] Write test `create_and_read_manifest` in `src/core/manifest.rs`: call `create_manifest` with a temp dir and a UUID string, then read the created `envy.toml` file and assert (1) it exists, (2) the `project_id` field equals the UUID passed in, (3) calling `find_manifest` on the same directory returns `Ok` with a matching `project_id`

### Implementation (T009–T012)

- [x] T009 [US1] Define the `Manifest` struct in `src/core/manifest.rs` with field `project_id: String` and `#[derive(serde::Serialize, serde::Deserialize)]`
- [x] T010 [US1] Implement `pub fn find_manifest(start_dir: &Path) -> Result<(Manifest, PathBuf), CoreError>` in `src/core/manifest.rs`: walk upward from `start_dir` checking for `envy.toml`; parse via `toml::from_str::<Manifest>`; map parse failures to `ManifestInvalid`, I/O errors to `ManifestIo`, and no-file-found to `ManifestNotFound`
- [x] T011 [US1] Implement `pub fn create_manifest(target_dir: &Path, project_id: &str) -> Result<(), CoreError>` in `src/core/manifest.rs`: serialise a `Manifest { project_id }` to TOML with a human-readable comment header; write to `<target_dir>/envy.toml`; map write errors (including file-already-exists) to `ManifestIo`
- [x] T012 Update `src/core/mod.rs` to add `mod manifest;` and `pub use manifest::{find_manifest, create_manifest, Manifest};`; run `cargo test` — all 4 manifest tests must pass

**Checkpoint**: `cargo test` — 4 new manifest tests pass; all prior tests still pass.

---

## Phase 4: Secret Operations — `ops.rs` (US2 + US3 + US4)

**Goal**: Implement the secret CRUD orchestrator, bulk environment decryption, and
environment defaulting/auto-creation. Covers User Stories 2 (encrypt/decrypt CRUD),
3 (bulk process injection), and 4 (environment defaulting).

**Independent Test**: `cargo test` passes all 11 tests in `src/core/ops.rs`.

### Tests — write first, verify they FAIL before implementation (T013–T023)

- [x] T013 [US2] Write test `set_and_get_secret_round_trip` in `src/core/ops.rs` (`#[cfg(test)]`): open a temp vault, call `set_secret` with a 32-byte all-zero key, project ID, env `"test"`, key `"API_KEY"`, plaintext `"secret123"`; then call `get_secret` with the same args; assert the returned value equals `"secret123"` and the raw DB blob (`value_encrypted`) does not contain the plaintext bytes
- [x] T014 [US2] Write test `set_secret_upsert` in `src/core/ops.rs`: call `set_secret` twice with the same key but different plaintext values (`"v1"` then `"v2"`); call `get_secret`; assert the returned value is `"v2"` (upsert semantics, last write wins)
- [x] T015 [US2] Write test `get_secret_not_found` in `src/core/ops.rs`: call `get_secret` for a key that was never set; assert `Err(CoreError::Db(_))` (NotFound propagated from DB layer)
- [x] T016 [US2] Write test `list_secret_keys_order` in `src/core/ops.rs`: store three secrets with keys `"ZEBRA"`, `"ALPHA"`, `"MANGO"` in a temp vault; call `list_secret_keys`; assert the returned `Vec<String>` is `["ALPHA", "MANGO", "ZEBRA"]` (alphabetical order) and has length 3 — no values or ciphertext in the result
- [x] T017 [US2] Write test `delete_secret_removes` in `src/core/ops.rs`: store a secret, call `delete_secret`, then call `get_secret` for the same key; assert the final result is `Err(CoreError::Db(_))`
- [x] T018 [US2] Write test `delete_secret_not_found` in `src/core/ops.rs`: call `delete_secret` on a key that was never set; assert `Err(CoreError::Db(_))`
- [x] T019 [US3] Write test `get_env_secrets_all_decrypted` in `src/core/ops.rs`: store three secrets (`"A"="1"`, `"B"="2"`, `"C"="3"`) in a temp vault; call `get_env_secrets`; assert the returned `HashMap` has exactly 3 entries with the correct decrypted values for each key
- [x] T020 [US3] Write test `get_env_secrets_empty_env` in `src/core/ops.rs`: create an environment in a temp vault but store no secrets; call `get_env_secrets`; assert `Ok(HashMap::new())` (empty map, not an error)
- [x] T021 [US3] Write test `get_env_secrets_partial_fail` in `src/core/ops.rs`: store two secrets normally, then manually corrupt one `value_encrypted` blob directly in the DB; call `get_env_secrets` with the correct master key; assert the entire call returns `Err(CoreError::Crypto(_))` — no partial map is returned
- [x] T022 [US4] Write test `default_env_auto_created` in `src/core/ops.rs`: on a freshly initialized project with zero environments, call `set_secret` with an empty `env_name` string; assert `Ok(())`; then query the DB and assert a `"development"` environment was auto-created and the secret is stored within it
- [x] T023 [US4] Write test `invalid_key_rejected` in `src/core/ops.rs`: call `set_secret` with an empty key `""`; assert `Err(CoreError::InvalidSecretKey(_))`; call `set_secret` with key `"FOO=BAR"` (contains `=`); assert `Err(CoreError::InvalidSecretKey(_))`; in both cases assert no DB or crypto call was made (environment row count stays zero)

### Implementation (T024–T031)

- [x] T024 Define `pub const DEFAULT_ENV: &str = "development";` and private `fn validate_key(key: &str) -> Result<(), CoreError>` (rejects empty or `=`-containing keys with `InvalidSecretKey`) in `src/core/ops.rs`
- [x] T025 [US4] Implement private `fn resolve_env(vault: &Vault, project_id: &ProjectId, env_name: &str) -> Result<EnvId, CoreError>` in `src/core/ops.rs`: lowercase the name; call `vault.get_environment_by_name`; on `DbError::NotFound` call `vault.create_environment` and return the new ID; propagate other errors as `CoreError::Db`
- [x] T026 [US2] Implement `pub fn set_secret(vault: &Vault, master_key: &[u8; 32], project_id: &ProjectId, env_name: &str, key: &str, plaintext: &str) -> Result<(), CoreError>` in `src/core/ops.rs`: validate key → resolve env (with auto-create) → `crypto::encrypt` → `vault.upsert_secret`
- [x] T027 [US2] Implement `pub fn get_secret(vault: &Vault, master_key: &[u8; 32], project_id: &ProjectId, env_name: &str, key: &str) -> Result<Zeroizing<String>, CoreError>` in `src/core/ops.rs`: validate key → resolve env (NO auto-create, use `get_environment_by_name` directly) → `vault.get_secret` → `crypto::decrypt` → `String::from_utf8` wrapped in `Zeroizing::new`
- [x] T028 [US2] Implement `pub fn list_secret_keys(vault: &Vault, project_id: &ProjectId, env_name: &str) -> Result<Vec<String>, CoreError>` in `src/core/ops.rs`: resolve env (no auto-create) → `vault.list_secrets` → extract `.key` field from each record → sort alphabetically
- [x] T029 [US2] Implement `pub fn delete_secret(vault: &Vault, project_id: &ProjectId, env_name: &str, key: &str) -> Result<(), CoreError>` in `src/core/ops.rs`: validate key → resolve env (no auto-create) → `vault.delete_secret`; propagate `NotFound` as-is
- [x] T030 [US3] Implement `pub fn get_env_secrets(vault: &Vault, master_key: &[u8; 32], project_id: &ProjectId, env_name: &str) -> Result<HashMap<String, Zeroizing<String>>, CoreError>` in `src/core/ops.rs`: resolve env (no auto-create) → `vault.list_secrets` → for each record `crypto::decrypt` → on any failure return error immediately (no partial map) → collect into `HashMap`
- [x] T031 Update `src/core/mod.rs` to add `mod ops;` and `pub use ops::{set_secret, get_secret, list_secret_keys, delete_secret, get_env_secrets, DEFAULT_ENV};`; run `cargo test` — all 11 ops tests must pass

**Checkpoint**: `cargo test` — 11 new ops tests pass; all prior tests still pass.

---

## Phase 5: Polish

**Purpose**: Quality gates, linting, and documentation update.

- [x] T032 [P] Run `cargo clippy -- -D warnings` and fix ALL warnings in `src/core/`
- [x] T033 [P] Run the full test suite `cargo test` — all tests (feature 001 + 002 + 003) must pass with 0 failures
- [x] T034 [P] Update `CLAUDE.md` active technologies line to include `toml = "0.8"` and `serde` (with `derive` feature) alongside the existing crates

**Checkpoint**: All 3 polish tasks pass. Feature 003 is complete.

---

## Dependencies & Execution Order

```
T001 → T002 → T003 → T004   (setup + error — strictly sequential)
                    ↓
         T005–T008           (manifest tests — write all, then...)
                    ↓
         T009 → T010 → T011 → T012   (manifest implementation — sequential within file)
                                   ↓
                        T013–T023   (ops tests — write all, then...)
                                   ↓
                        T024 → T025 → T026 → T027 → T028 → T029 → T030 → T031
                                                                           ↓
                                              T032 [P]  T033 [P]  T034 [P]
```

### Phase dependencies

- **Phase 1** (T001–T002): No dependencies — start immediately
- **Phase 2** (T003–T004): Requires Phase 1 — `CoreError` is used in manifest + ops tests
- **Phase 3** (T005–T012): Requires Phase 2 — tests import `CoreError`; uses `tempfile` dev-dep
- **Phase 4** (T013–T031): Requires Phase 2 — tests import `CoreError`; can start in parallel with Phase 3 if needed (different file), but sequential is safer
- **Phase 5** (T032–T034): Requires Phases 3 and 4 complete

---

## Implementation Strategy

### Single-developer sequence (recommended)

1. Phase 1 → 2 → 3 (tests then implementation) → 4 (tests then implementation) → 5
2. Run `cargo test` after T012 and after T031 as interim checkpoints
3. Phase 5 is the final gate before marking the feature complete

### Parallel opportunities

- T005–T008 (manifest tests) can all be written in a single pass before any implementation
- T013–T023 (ops tests) can all be written in a single pass before any implementation
- T032, T033, T034 (polish) are independent and can run in any order