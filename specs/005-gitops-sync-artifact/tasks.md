# Tasks: GitOps Sync Artifact (`envy.enc`)

**Input**: Design documents from `/specs/005-gitops-sync-artifact/`
**Prerequisites**: spec.md ✓ | plan.md ✓ | research.md ✓ | data-model.md ✓ | contracts/crypto-artifact.md ✓

**Approach**: Strict TDD — every test task MUST be written and verified to compile before its implementation task begins. Tests MUST fail before implementation. Tests MUST pass after.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel with other [P] tasks in the same phase
- **[US1–US4]**: Maps to User Story in spec.md

---

## Phase 1: Setup

**Purpose**: Add new dependencies and create scaffolded (empty) module files so all subsequent phases compile cleanly.

- [x] T001 Add `argon2 = "0.5"`, `serde_json = "1"`, `base64ct = { version = "1", features = ["alloc"] }` to `[dependencies]` in `Cargo.toml`
- [x] T002 Create `src/crypto/artifact.rs` with module-level `//!` doc comment and a single placeholder `pub struct SyncArtifact;` so the file compiles
- [x] T003 Create `src/core/sync.rs` with module-level `//!` doc comment and a single placeholder `pub struct UnsealResult;` so the file compiles
- [x] T004 Add `mod artifact;` to `src/crypto/mod.rs` and `mod sync;` to `src/core/mod.rs`; run `cargo build` and confirm zero errors (dead-code warnings are acceptable at this stage)

---

## Phase 2: Crypto Primitives — `src/crypto/artifact.rs` (TDD)

**Purpose**: All low-level types, constants, error enum, `derive_key`, `seal_envelope`, `unseal_envelope`.

**Goal**: After this phase, all 6 unit tests in `src/crypto/artifact.rs` pass.

**Independent Test**: `cargo test --test-threads=1 -- crypto::artifact` — all 6 tests green.

### Tests for Phase 2 (write first — MUST fail before T014)

- [x] T005 [P] [US1] Write test `derive_key_round_trip` in `src/crypto/artifact.rs`: call `derive_key` twice with the same passphrase and salt, assert both results are byte-equal
- [x] T006 [P] [US1] Write test `derive_key_different_salts_produce_different_keys` in `src/crypto/artifact.rs`: call `derive_key` with the same passphrase but two different salts, assert results differ
- [x] T007 [P] [US1] Write test `seal_unseal_envelope_round_trip` in `src/crypto/artifact.rs`: seal a known `ArtifactPayload`, unseal with the same passphrase, assert the recovered secrets map matches the original byte-for-byte
- [x] T008 [P] [US2] Write test `wrong_passphrase_returns_malformed_envelope` in `src/crypto/artifact.rs`: seal an envelope with passphrase `"correct"`, attempt to unseal with `"wrong"`, assert `Err(ArtifactError::MalformedEnvelope(_, _))`
- [x] T009 [P] [US4] Write test `tampered_ciphertext_returns_malformed_envelope` in `src/crypto/artifact.rs`: seal an envelope, Base64-decode the ciphertext field, flip one byte (`ciphertext_bytes[0] ^= 0xFF`), re-encode and unseal with the correct passphrase, assert `Err(ArtifactError::MalformedEnvelope(_, _))`
- [x] T010 [P] [US1] Write test `empty_passphrase_returns_weak_passphrase` in `src/crypto/artifact.rs`: call `seal_envelope` with `""` and with `"   "` (whitespace), assert both return `Err(ArtifactError::WeakPassphrase)`
- [x] T011 Run `cargo test --no-run` to verify all 6 tests in T005–T010 compile (expected: all fail with "unresolved" or "not found" errors, NOT compile errors)

### Implementation for Phase 2

- [x] T012 Implement `ArtifactError` enum with all 5 variants (`WeakPassphrase`, `MalformedArtifact`, `MalformedEnvelope`, `UnsupportedVersion`, `KdfFailed`) and `ARTIFACT_VERSION`, `KDF_MEMORY_KIB`, `KDF_TIME_COST`, `KDF_PARALLELISM`, `KDF_SALT_BYTES` constants in `src/crypto/artifact.rs`
- [x] T013 Implement `KdfParams`, `EncryptedEnvelope`, `SyncArtifact` (all `#[derive(Debug, Clone, Serialize, Deserialize)]` with `BTreeMap` for environments), and `ArtifactPayload` (with `BTreeMap<String, Zeroizing<String>>`) in `src/crypto/artifact.rs`
- [x] T014 Implement `derive_key(passphrase, salt, params) -> Result<Zeroizing<[u8; 32]>, ArtifactError>` in `src/crypto/artifact.rs`: validate non-empty passphrase, build Argon2id context from params, call `argon2::Argon2::new(...).hash_password_into(...)`, wrap in `Zeroizing`
- [x] T015 Implement `seal_envelope(passphrase, payload) -> Result<EncryptedEnvelope, ArtifactError>` in `src/crypto/artifact.rs`: validate passphrase, generate 16-byte OsRng salt, build `KdfParams`, call `derive_key`, serialize `payload.secrets` to JSON bytes, call existing `crate::crypto::encrypt`, Base64-encode ciphertext and nonce, return `EncryptedEnvelope`
- [x] T016 Implement `unseal_envelope(passphrase, env_name, envelope) -> Result<ArtifactPayload, ArtifactError>` in `src/crypto/artifact.rs`: validate passphrase, check `kdf.algorithm == "argon2id"`, Base64-decode salt/nonce/ciphertext, call `derive_key`, call existing `crate::crypto::decrypt` (map `DecryptionFailed` → `MalformedEnvelope(env_name, "authentication failed")`), deserialize JSON bytes to `BTreeMap<String, String>`, wrap values in `Zeroizing<String>`
- [x] T017 Update `src/crypto/mod.rs`: add `pub use artifact::{ArtifactError, ArtifactPayload, EncryptedEnvelope, KdfParams, SyncArtifact, ARTIFACT_VERSION, KDF_MEMORY_KIB, KDF_PARALLELISM, KDF_SALT_BYTES, KDF_TIME_COST, derive_key, seal_envelope, unseal_envelope};`
- [x] T018 Run `cargo test -- crypto::artifact` and confirm all 6 tests (T005–T010) pass, 0 fail

**Checkpoint**: `seal_envelope` / `unseal_envelope` / `derive_key` are fully tested and working.

---

## Phase 3: Core Orchestration — `src/core/sync.rs` (TDD)

**Purpose**: `SyncError`, `UnsealResult`, `seal_artifact`, `unseal_artifact`, `write_artifact`, `read_artifact`.

**Goal**: After this phase, all 5 unit tests in `src/core/sync.rs` pass.

**Independent Test**: `cargo test -- core::sync` — all 5 tests green.

### Tests for Phase 3 (write first — MUST fail before T025)

- [x] T019 [P] [US1] Write test `seal_artifact_produces_valid_json_structure` in `src/core/sync.rs`: use a `tempfile` vault with known secrets in `development`, call `seal_artifact`, assert the returned `SyncArtifact` has `version == 1`, has an `"development"` key in `environments`, and that the `EncryptedEnvelope` fields (`ciphertext`, `nonce`, `kdf.algorithm`) are all non-empty
- [x] T020 [P] [US3] Write test `unseal_artifact_progressive_disclosure` in `src/core/sync.rs`: seal two envelopes (`"development"` with `"dev-pass"`, `"production"` with `"prod-pass"`) directly into a `SyncArtifact`, call `unseal_artifact` with `"dev-pass"`, assert `result.imported` contains `"development"` and `result.skipped` contains `"production"`
- [x] T021 [P] [US1] Write test `write_read_artifact_round_trip` in `src/core/sync.rs`: seal an artifact, `write_artifact` to a `tempfile` path, `read_artifact` from the same path, assert the re-read artifact has the same `version` and the same environment keys
- [x] T022 [P] [US4] Write test `read_artifact_malformed_json_returns_error` in `src/core/sync.rs`: write the string `"not json at all"` to a temp file, call `read_artifact`, assert `Err(SyncError::Artifact(ArtifactError::MalformedArtifact(_)))`
- [x] T023 [P] [US4] Write test `read_artifact_unknown_version_returns_error` in `src/core/sync.rs`: write a JSON object `{"version": 999, "environments": {}}` to a temp file, call `read_artifact`, assert `Err(SyncError::UnsupportedVersion(999))`
- [x] T024 Run `cargo test --no-run` to verify all 5 tests in T019–T023 compile cleanly

### Implementation for Phase 3

- [x] T025 Implement `SyncError` enum with all 5 variants (`Artifact(#[from] ArtifactError)`, `FileNotFound`, `Io`, `UnsupportedVersion`, `NothingImported`) in `src/core/sync.rs`
- [x] T026 Implement `UnsealResult` struct (`imported: BTreeMap<String, BTreeMap<String, Zeroizing<String>>>`, `skipped: Vec<String>`) in `src/core/sync.rs`
- [x] T027 Implement `seal_artifact(vault, master_key, project_id, passphrase, envs) -> Result<SyncArtifact, SyncError>` in `src/core/sync.rs`: validate passphrase early, determine env names (all or filtered), for each env call `core::get_env_secrets`, build `ArtifactPayload`, call `seal_envelope`, collect into `BTreeMap`, return `SyncArtifact { version: ARTIFACT_VERSION, environments }`
- [x] T028 Implement `unseal_artifact(artifact, passphrase) -> Result<UnsealResult, SyncError>` in `src/core/sync.rs`: validate passphrase, check `artifact.version == ARTIFACT_VERSION`, iterate environments, call `unseal_envelope` for each, on `Ok` insert into `imported`, on `Err(_)` push to `skipped` (Progressive Disclosure — ALL errors → skip, never abort), return `Ok(UnsealResult { imported, skipped })`
- [x] T029 Implement `write_artifact(artifact, path) -> Result<(), SyncError>` in `src/core/sync.rs`: call `serde_json::to_string_pretty(artifact)`, write to path with `fs::write`, map errors to `SyncError::Io`
- [x] T030 Implement `read_artifact(path) -> Result<SyncArtifact, SyncError>` in `src/core/sync.rs`: check path exists (else `FileNotFound`), `fs::read_to_string`, `serde_json::from_str::<SyncArtifact>` (else `MalformedArtifact`), check `artifact.version == ARTIFACT_VERSION` (else `UnsupportedVersion`), return `Ok(artifact)`
- [x] T031 Update `src/core/mod.rs`: add `pub use sync::{SyncError, UnsealResult, seal_artifact, unseal_artifact, write_artifact, read_artifact};`
- [x] T032 Run `cargo test -- core::sync` and confirm all 5 tests (T019–T023) pass, 0 fail

**Checkpoint**: Full seal/unseal pipeline works end-to-end. Progressive Disclosure verified.

---

## Phase 4: Integration Tests — `tests/sync_artifact.rs`

**Purpose**: End-to-end flows using real vault and `tempfile` isolation. No mocks.

**Goal**: All 5 integration tests compile and pass with `cargo test`.

### Integration Tests

- [x] T033 Create `tests/sync_artifact.rs` with a `setup_test_vault(tmp: &TempDir) -> (Vault, [u8; 32], ProjectId)` helper that opens a vault in the temp dir, creates a project, and returns all three values — mirrors the pattern used in `tests/cli_integration.rs`
- [x] T034 [US1] Write integration test `e2e_seal_and_unseal_full_vault` in `tests/sync_artifact.rs`: `set_secret` for `STRIPE_KEY` in `development` and `DB_URL` in `production`, call `seal_artifact` with a passphrase, then `unseal_artifact` with the same passphrase, assert both environments are in `imported` and values match exactly
- [x] T035 [US2] Write integration test `e2e_wrong_passphrase_skips_all_environments` in `tests/sync_artifact.rs`: seal a vault with passphrase `"correct"`, unseal with `"wrong"`, assert `result.imported` is empty and `result.skipped` contains all environment names (no hard error returned)
- [x] T036 [US3] Write integration test `e2e_partial_access_progressive_disclosure` in `tests/sync_artifact.rs`: seal `development` into an artifact with `"dev-key"` and `production` with `"prod-key"` (construct `SyncArtifact` manually with two separately sealed envelopes), call `unseal_artifact` with `"dev-key"`, assert `"development"` is imported and `"production"` is in `skipped`
- [x] T037 [US4] Write integration test `e2e_tampered_ciphertext_skips_environment` in `tests/sync_artifact.rs`: seal an artifact, decode the ciphertext Base64 of `"development"`, flip bit 0 of byte 0, re-encode, replace the envelope, call `unseal_artifact` with the correct passphrase, assert `"development"` is in `skipped` (not an error) and vault is untouched
- [x] T038 [US1] Write integration test `e2e_write_read_artifact_file_round_trip` in `tests/sync_artifact.rs`: `seal_artifact`, `write_artifact` to a temp path, `read_artifact` from the same path, assert the parsed `SyncArtifact` has `version == 1` and the same environment keys as the original
- [x] T039 Run `cargo test --no-run` to verify all integration tests compile
- [x] T040 Run `cargo test --test sync_artifact` and confirm all 5 integration tests (T034–T038) pass, 0 fail

**Checkpoint**: Full end-to-end pipeline validated against a real vault with tempfile isolation.

---

## Phase 5: Polish

**Purpose**: Zero warnings, consistent formatting, full baseline test suite passes.

- [x] T041 Run `cargo clippy -- -D warnings` and fix all warnings in `src/crypto/artifact.rs` and `src/core/sync.rs`
- [x] T042 Run `cargo fmt` to apply standard Rust formatting across all modified files
- [x] T043 Run `cargo test` (full suite — all unit + integration tests) and confirm the baseline passes with 0 failures
- [x] T044 Update `CLAUDE.md` to record the new `005-gitops-sync-artifact` technology: `argon2 = "0.5"` (Argon2id KDF), `serde_json = "1"` (JSON artifact serialization), `base64ct = "1"` (constant-time Base64), and the two new modules `src/crypto/artifact.rs` and `src/core/sync.rs`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Crypto Primitives)**: Requires Phase 1 complete
- **Phase 3 (Core Orchestration)**: Requires Phase 2 complete (calls `seal_envelope`, `unseal_envelope`)
- **Phase 4 (Integration Tests)**: Requires Phase 3 complete (uses full pipeline)
- **Phase 5 (Polish)**: Requires Phase 4 complete

### Within Each Phase

1. All `[P]`-marked test tasks can be written in parallel (different test functions, same file)
2. Tests MUST be written and verified to compile (`--no-run`) before implementation begins
3. Implementation tasks within a phase are sequential (types before functions, error enum before functions)

### Parallel Opportunities

```bash
# Phase 2 tests — all 6 can be drafted in parallel (different test functions):
T005, T006, T007, T008, T009, T010

# Phase 3 tests — all 5 can be drafted in parallel:
T019, T020, T021, T022, T023

# Phase 4 integration tests — all 5 can be drafted in parallel:
T034, T035, T036, T037, T038
```

---

## Test Coverage Summary

All 11 test cases from `contracts/crypto-artifact.md` are assigned to specific tasks:

| Contract Test | Task |
|---|---|
| `derive_key` round-trip (same passphrase + salt → same key) | T005 |
| `derive_key` different salts → different keys | T006 |
| `seal_envelope` / `unseal_envelope` round-trip | T007 |
| Wrong passphrase → `MalformedEnvelope` | T008 |
| Tampered ciphertext (bit flip) → `MalformedEnvelope` | T009 |
| Empty passphrase → `WeakPassphrase` | T010 |
| `seal_artifact` produces valid JSON with correct structure | T019 |
| `unseal_artifact` skips inaccessible envs (Progressive Disclosure) | T020 |
| `write_artifact` / `read_artifact` round-trip | T021 |
| Malformed JSON → `MalformedArtifact` | T022 |
| Unknown `version` → `UnsupportedVersion` | T023 |

Additional integration tests (Phase 4): T034–T038

---

## Implementation Strategy

### MVP Scope (US1 only — Startup Mode)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (Crypto Primitives)
3. Complete Phase 3 (Core Orchestration)
4. **STOP** — `seal_artifact` / `unseal_artifact` with a shared passphrase is fully functional
5. Phase 4 and Phase 5 complete the feature

### Incremental Delivery

- After Phase 2: `seal_envelope` / `unseal_envelope` work — crypto layer is auditable independently
- After Phase 3: Full artifact pipeline works — CLI layer (feature 006) can now call `seal_artifact` / `unseal_artifact`
- After Phase 4: Integration validated — ready to commit
- After Phase 5: Production-quality code — ready to merge to `master`

---

## Notes

- Argon2id at 64 MiB × 3 environments = ~1.5 s on commodity hardware — normal for this security level
- Integration tests (T034–T038) do NOT require OS keyring: the vault is opened with a raw `[u8; 32]` key
- The `--test-threads=1` flag may be needed for integration tests that share temp directories
- `NothingImported` is NOT returned by `unseal_artifact` itself — it is a `SyncError` variant reserved for the CLI layer (feature 006) to surface when appropriate