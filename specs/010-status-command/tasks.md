# Tasks: Vault Sync Status Command

**Input**: Design documents from `specs/010-status-command/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/status-command.md ✓, quickstart.md ✓

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US4)
- All file paths are relative to the repository root

---

## Phase 1: Setup

**Purpose**: Add the one new dependency before any code is written.

- [x] T001 Add `comfy-table = "7"` to `[dependencies]` in `Cargo.toml` (plan.md §3.1 — table rendering for cmd_status)

**Checkpoint**: `cargo check` must pass before Phase 2.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: DB layer and Core layer infrastructure that ALL user story implementations depend on. Must be complete before any story phase begins.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete and `cargo test` passes.

### Milestone 1 — Database Layer

- [x] T002 Update `src/db/schema.rs`: add `SCHEMA_V2` const with `CREATE TABLE IF NOT EXISTS sync_markers (environment_id TEXT NOT NULL PRIMARY KEY REFERENCES environments(id) ON DELETE CASCADE, sealed_at INTEGER NOT NULL)`, update `run_migrations` to apply V2 when `version == 1` (sets `user_version = 2`), update module comment to document `1 → 2` step (plan.md §1.1; data-model.md §Schema V2)
- [x] T003 Create `src/db/sync_markers.rs`: define `pub struct EnvironmentStatus { name: String, secret_count: i64, last_modified_at: Option<i64>, sealed_at: Option<i64> }` with `#[derive(Debug, Clone)]` (plan.md §1.3; data-model.md §EnvironmentStatus)
- [x] T004 Add `pub fn upsert_sync_marker(&self, env_id: &EnvId, sealed_at: i64) -> Result<(), DbError>` to the `impl Vault` block in `src/db/sync_markers.rs`: executes `INSERT OR REPLACE INTO sync_markers (environment_id, sealed_at) VALUES (?1, ?2)` (plan.md §1.2; research.md §Decision 4)
- [x] T005 Add `pub fn environment_status(&self, project_id: &ProjectId) -> Result<Vec<EnvironmentStatus>, DbError>` to the `impl Vault` block in `src/db/sync_markers.rs`: executes the LEFT JOIN aggregation query joining `environments`, `secrets`, and `sync_markers` grouped by `e.id`, ordered by `e.name ASC`, mapping NULL `last_modified_at` to `Option<i64>` (plan.md §1.3)
- [x] T006 Update `src/db/mod.rs`: add `mod sync_markers;` and `pub use sync_markers::EnvironmentStatus;` alongside existing re-exports (plan.md §1.4)
- [x] T007 Add unit test `schema_v2_migration_adds_sync_markers_table` in `src/db/sync_markers.rs` `#[cfg(test)]` block: open vault, assert `vault.table_exists("sync_markers")` returns `true` (plan.md §1.5)
- [x] T008 Add unit test `upsert_sync_marker_creates_row` in `src/db/sync_markers.rs`: create project + environment, call `upsert_sync_marker(&env_id, 1000)`, query `SELECT sealed_at FROM sync_markers WHERE environment_id = ?`, assert result equals `1000` (plan.md §1.5)
- [x] T009 Add unit test `upsert_sync_marker_updates_existing_row` in `src/db/sync_markers.rs`: upsert with timestamp `1000`, upsert again with `2000`, assert only one row exists with `sealed_at = 2000` (plan.md §1.5; research.md §Decision 4)
- [x] T010 Add unit test `environment_status_returns_never_sealed_when_no_marker` in `src/db/sync_markers.rs`: create env + secret, do NOT upsert sync marker, call `environment_status`, assert `sealed_at = None` and `secret_count = 1` (plan.md §1.5)
- [x] T011 Add unit test `environment_status_returns_correct_last_modified_at` in `src/db/sync_markers.rs`: create env + secret, call `environment_status`, assert `last_modified_at` is `Some` and equals the secret's `updated_at` (plan.md §1.5)
- [x] T012 Add unit test `environment_status_zero_secrets_returns_none_last_modified` in `src/db/sync_markers.rs`: create env with no secrets, call `environment_status`, assert `secret_count = 0` and `last_modified_at = None` (plan.md §1.5)
- [x] T013 Add unit test `environment_status_returns_multiple_envs_sorted_by_name` in `src/db/sync_markers.rs`: create envs named `"zebra"` and `"alpha"`, call `environment_status`, assert result is ordered `["alpha", "zebra"]` (plan.md §1.5; contracts §sorting)
- [x] T014 Add unit test `sync_marker_deleted_on_environment_cascade` in `src/db/sync_markers.rs`: create env, upsert sync marker, delete environment, query `sync_markers` — assert zero rows remain (plan.md §1.5; data-model.md §ON DELETE CASCADE)

**Checkpoint**: `cargo test` must pass (all DB tests green) before Milestone 2.

### Milestone 2 — Core Layer

- [x] T015 Create `src/core/status.rs`: define `pub enum SyncStatus { InSync, Modified, NeverSealed }` with `#[derive(Debug, Clone, PartialEq, Eq)]` and `pub struct StatusRow { pub name: String, pub secret_count: i64, pub last_modified_at: Option<i64>, pub sealed_at: Option<i64>, pub sync_status: SyncStatus }` (plan.md §2.1; research.md §Decision 5)
- [x] T016 Add `pub fn derive_sync_status(secret_count: i64, last_modified_at: Option<i64>, sealed_at: Option<i64>) -> SyncStatus` to `src/core/status.rs`: `sealed_at = None → NeverSealed`; `last_modified_at = None → InSync`; `last_modified_at > sealed_at → Modified`; else `InSync` (plan.md §2.1; data-model.md §SyncStatus)
- [x] T017 Add `pub fn get_status_report(vault: &Vault, project_id: &ProjectId) -> Result<Vec<StatusRow>, CoreError>` to `src/core/status.rs`: calls `vault.environment_status(project_id)`, maps each `EnvironmentStatus` to `StatusRow` via `derive_sync_status`, returns sorted `Vec<StatusRow>` (plan.md §2.1)
- [x] T018 Update `src/core/mod.rs`: add `pub mod status;` and `pub use status::{SyncStatus, StatusRow, get_status_report};` alongside existing re-exports (plan.md §2.3)
- [x] T019 Add unit test `derive_sync_status_never_sealed` in `src/core/status.rs`: `sealed_at = None` → `NeverSealed` regardless of secret_count/last_modified_at (plan.md §2.4)
- [x] T020 Add unit test `derive_sync_status_in_sync` in `src/core/status.rs`: `sealed_at = Some(100), last_modified_at = Some(90)` → `InSync` (plan.md §2.4)
- [x] T021 Add unit test `derive_sync_status_modified` in `src/core/status.rs`: `sealed_at = Some(90), last_modified_at = Some(100)` → `Modified` (plan.md §2.4)
- [x] T022 Add unit test `derive_sync_status_no_secrets_with_marker` in `src/core/status.rs`: `sealed_at = Some(100), last_modified_at = None, secret_count = 0` → `InSync` (plan.md §2.4; data-model.md §edge case)
- [x] T023 Add unit test `derive_sync_status_equal_timestamps` in `src/core/status.rs`: `sealed_at = Some(100), last_modified_at = Some(100)` → `InSync` (boundary condition: `<=` not `<`) (plan.md §2.4; spec.md FR-002)

**Checkpoint**: `cargo test` must pass (all DB + Core tests green) before Phase 3.

---

## Phase 3: User Story 1 — Instant Sync Awareness (Priority: P1) 🎯 MVP

**Goal**: `envy status` renders a human-readable table of all environments with their sync status, secret count, and relative last-modified time. No passphrase. No decryption.

**Independent Test**: Create a vault with three environments: one sealed (via direct `upsert_sync_marker`), one with a newer secret after sealing, one never sealed. Run `cmd_status`. Assert correct status labels for each.

- [x] T024 [US1] Add `Status` variant to `Commands` enum in `src/cli/mod.rs` with doc comment matching contracts/status-command.md; add dispatch arm in `run()` that calls `commands::cmd_status(&vault, &project_id, &artifact, cli.format)` and maps errors (plan.md §3.2)
- [x] T025 [US1] Add private `fn humanize_timestamp(epoch: i64) -> String` to `src/cli/commands.rs`: computes `now_unix - epoch`, returns `"X seconds ago"`, `"X minutes ago"`, `"X hours ago"`, `"X days ago"` for respective thresholds, ISO date (`YYYY-MM-DD`) for older, `"unknown"` for zero or negative diff (plan.md §3.3; contracts §Last Modified)
- [x] T026 [US1] Add `pub(super) fn cmd_status(vault: &Vault, project_id: &ProjectId, artifact_path: &Path, format: OutputFormat) -> Result<(), CliError>` to `src/cli/commands.rs` — table path only: calls `crate::core::get_status_report`, if empty prints "No environments found. Use 'envy set' to add secrets first." and returns `Ok(())`; otherwise builds `comfy_table::Table` with header `["Environment", "Secrets", "Last Modified", "Status"]`, one row per `StatusRow` with colorised status cell (`✓ In Sync` green / `⚠ Modified` yellow / `✗ Never Sealed` red), `println!("{table}")` (plan.md §3.3; contracts §table format)
- [x] T027 [P] [US1] Add unit test `status_shows_never_sealed_for_new_environment` in `src/cli/commands.rs`: create vault, add secret to `"development"`, call `cmd_status` with `OutputFormat::Table`, assert `Ok(())` (plan.md §3.6)
- [x] T028 [P] [US1] Add unit test `status_shows_in_sync_via_direct_db_marker` in `src/cli/commands.rs`: create vault + secret, call `vault.upsert_sync_marker(&env_id, past_timestamp)`, call `cmd_status`, assert `Ok(())` (plan.md §3.6)
- [x] T029 [P] [US1] Add unit test `status_empty_vault_returns_ok` in `src/cli/commands.rs`: create vault with no environments, call `cmd_status`, assert `Ok(())` (spec.md US1 Acceptance Scenario 4)
- [x] T030 [P] [US1] Add unit test `humanize_timestamp_seconds` in `src/cli/commands.rs`: `now - 30 seconds` → contains `"seconds ago"` (plan.md §3.3)
- [x] T031 [P] [US1] Add unit test `humanize_timestamp_minutes` in `src/cli/commands.rs`: `now - 90 seconds` → contains `"minutes ago"` (plan.md §3.3)
- [x] T032 [P] [US1] Add unit test `humanize_timestamp_days` in `src/cli/commands.rs`: `now - 3 * 86400 seconds` → contains `"days ago"` (plan.md §3.3)

**Checkpoint**: `cargo test` passes. `envy status` renders a table for any vault state with correct sync labels.

---

## Phase 4: User Story 2 — Sync State Stays Accurate After Encrypting (Priority: P2)

**Goal**: Every successful `envy encrypt` updates the `sync_markers` table. Running `envy status` immediately after shows the sealed environment as "In Sync".

**Independent Test**: Call `seal_env` (or `cmd_encrypt`) for `development`; call `vault.environment_status`; assert `sealed_at` is `Some` and recent. Then call `cmd_status`; assert "In Sync" for `development`.

- [x] T033 [US2] Update `pub fn seal_env` in `src/core/sync.rs`: after `Ok(seal_envelope(passphrase, &payload)?)` succeeds, call `vault.get_environment_by_name(project_id, env_name)` to obtain the `EnvId`, compute `now` via `std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64` (with inline SAFETY comment justifying `unwrap_or_default`), then call `vault.upsert_sync_marker(&env_id, now)`, propagating errors as `SyncError::VaultError`; the envelope is returned only if the marker write also succeeds (plan.md §2.2; spec.md FR-008)
- [x] T034 [US2] Add unit test `seal_env_writes_sync_marker` in `src/core/sync.rs`: create vault + secret for `"development"`, call `seal_env`, then call `vault.environment_status`, assert the `development` row has `sealed_at = Some(_)` with a non-zero value (plan.md §2.4)
- [x] T035 [US2] Add unit test `status_shows_in_sync_after_encrypt` in `src/cli/commands.rs`: create vault + secret, run `cmd_encrypt` with `ENVY_PASSPHRASE`, then run `cmd_status`, assert `Ok(())` — verifies full round-trip sync marker write (spec.md US2 Acceptance Scenario 1)
- [x] T036 [US2] Add unit test `status_shows_modified_after_set` in `src/cli/commands.rs`: encrypt `development`, then call `crate::core::set_secret` to add a new key, then call `cmd_status`, assert `Ok(())` — verifies "Modified" state is detectable after encrypt+set cycle (spec.md US2 Acceptance Scenario 2)

**Checkpoint**: `cargo test` passes. Sync state is live — reflects vault changes immediately after encrypt.

---

## Phase 5: User Story 3 — Machine-Readable JSON Output (Priority: P3)

**Goal**: `envy status --format json` outputs a single valid JSON object with `environments` array and `artifact` object to stdout, suitable for CI/CD pipeline parsing.

**Independent Test**: Call `cmd_status` with `OutputFormat::Json`, capture stdout via `Vec<u8>`, assert valid JSON with `status` strings matching `"in_sync"`, `"modified"`, or `"never_sealed"`.

- [x] T037 [US3] Add private serde structs `StatusJson`, `EnvStatusJson`, `ArtifactJson` to `src/cli/commands.rs` matching the contracts/status-command.md JSON schema: `status` field is `&'static str` (`"in_sync"`, `"modified"`, `"never_sealed"`), timestamps are `Option<String>` (ISO 8601 UTC), `found` is `bool` (plan.md §3.5; contracts §JSON fields)
- [x] T038 [US3] Add private `fn epoch_to_iso8601(secs: i64) -> String` to `src/cli/commands.rs`: converts Unix epoch seconds to `"YYYY-MM-DDTHH:MM:SSZ"` string using pure `std::time` arithmetic without any `chrono` dependency; returns `"1970-01-01T00:00:00Z"` for zero (plan.md §3.5; research.md §Decision 2)
- [x] T039 [US3] Add JSON rendering branch to `cmd_status` in `src/cli/commands.rs`: when `format == OutputFormat::Json`, build `StatusJson` from status rows and artifact metadata, serialize via `serde_json::to_writer` to stdout, write trailing newline — the `artifact` object in the JSON path uses an empty `ArtifactJson` with `found = false` until Phase 6 wires the artifact metadata (plan.md §3.3; contracts §JSON output)
- [x] T040 [P] [US3] Add unit test `status_json_output_is_valid_json` in `src/cli/commands.rs`: create vault + secret, call `cmd_status` with `OutputFormat::Json` capturing stdout, assert `serde_json::from_str` succeeds and the `environments` array has length 1 (plan.md §3.6; spec.md US3 Acceptance Scenario 1)
- [x] T041 [P] [US3] Add unit test `status_json_status_strings_are_lowercase` in `src/cli/commands.rs`: seal one env, add secret after seal, call `cmd_status --format json`, parse JSON, assert the modified env has `status == "modified"` (not `"Modified"`) (plan.md §3.6; contracts §status field; spec.md US3 Acceptance Scenario 2)
- [x] T042 [P] [US3] Add unit test `epoch_to_iso8601_known_value` in `src/cli/commands.rs`: assert `epoch_to_iso8601(0) == "1970-01-01T00:00:00Z"` and `epoch_to_iso8601(1000000000) == "2001-09-09T01:46:40Z"` (plan.md §3.5)

**Checkpoint**: `cargo test` passes. `envy status --format json` produces machine-parseable output.

---

## Phase 6: User Story 4 — Artifact Metadata Visibility (Priority: P4)

**Goal**: Both the table and JSON outputs include a section showing which environments are sealed in `envy.enc`, the file's last-write time, and a warning when an artifact environment is absent from the local vault. No passphrase. No decryption.

**Independent Test**: Write a valid `envy.enc` with two environment names, call `cmd_status`, assert both names appear in the artifact section of the output without any passphrase prompt.

- [x] T043 [US4] Add private `fn read_artifact_metadata(artifact_path: &Path) -> ArtifactMetadata` to `src/cli/commands.rs` where `ArtifactMetadata` is a private struct with `found: bool, last_modified_at: Option<i64>, environments: Vec<String>`; reads the file mtime via `std::fs::metadata(path)?.modified()`, deserializes the JSON via `crate::core::read_artifact` to extract env names — `found = false` if file missing, `environments = vec![]` if malformed (plan.md §3.4; research.md §Decision 6; spec.md FR-003)
- [x] T044 [US4] Extend `cmd_status` table path in `src/cli/commands.rs` to call `read_artifact_metadata` and print the artifact section after the environment table: `"Artifact: {path}  (last written: {humanize})\n  Sealed environments: {names}"` — or `"not found"` / `"unreadable (malformed JSON)"` for error states; additionally print `"  ⚠  {name} is in the artifact but not in the local vault"` for any artifact env name not present in the vault status rows (plan.md §3.4; contracts §artifact section; quickstart.md §Scenario 7)
- [x] T045 [US4] Extend `cmd_status` JSON path in `src/cli/commands.rs` to populate the `ArtifactJson` field in `StatusJson` using the `ArtifactMetadata` from `read_artifact_metadata` (replacing the stub from T039): `found`, `path`, `last_modified_at` (via `epoch_to_iso8601`), `environments` list (plan.md §3.3; contracts §artifact JSON fields; spec.md FR-007)
- [x] T046 [P] [US4] Add unit test `status_artifact_not_found_renders_gracefully` in `src/cli/commands.rs`: call `cmd_status` where the artifact path does not exist, assert `Ok(())` — verifies exit 0 on missing artifact (spec.md US1 Acceptance Scenario 5; contracts §exit codes)
- [x] T047 [P] [US4] Add unit test `status_artifact_malformed_renders_gracefully` in `src/cli/commands.rs`: write `b"not-valid-json"` to the artifact path, call `cmd_status`, assert `Ok(())` — verifies graceful degradation on malformed artifact (spec.md Edge Cases; contracts §artifact section)
- [x] T048 [P] [US4] Add unit test `status_json_artifact_found_false_when_missing` in `src/cli/commands.rs`: call `cmd_status` with `OutputFormat::Json` where artifact path does not exist, parse JSON, assert `artifact.found == false` and `artifact.environments` is an empty array (spec.md US3 Acceptance Scenario 3)

**Checkpoint**: `cargo test` passes. All 4 user stories are fully functional and independently verified.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [x] T049 [P] Run `cargo clippy -- -D warnings` and fix any new warnings introduced by this feature across all modified files (`Cargo.toml`, `src/db/schema.rs`, `src/db/sync_markers.rs`, `src/db/mod.rs`, `src/core/status.rs`, `src/core/sync.rs`, `src/core/mod.rs`, `src/cli/mod.rs`, `src/cli/commands.rs`)
- [x] T050 [P] Run `cargo fmt` on all modified files and verify no diff remains
- [x] T051 [P] Run `cargo audit` and confirm no new CVEs introduced by `comfy-table = "7"` — document result in a comment if any existing advisories remain unchanged
- [x] T052 Add E2E test Scenario 8 to `tests/e2e_devops_scenarios.sh`: set `ENVY_PASSPHRASE_DEVELOPMENT` + `ENVY_PASSPHRASE_PRODUCTION`, run `envy encrypt`, run `envy status`, assert output contains `"In Sync"` for both environments (quickstart.md §E2E assertions; spec.md SC-004)
- [x] T053 [P] Update `CLAUDE.md` tech stack section: add `comfy-table = "7"` entry under `010-status-command` alongside the existing technology lines (plan.md §T-POLISH-5)

---

## Dependency Graph

```
T001 (Cargo.toml)
  │
  ▼
T002 (schema.rs V2)
  │
  ▼
T003–T006 (sync_markers.rs + mod.rs)
  │
  ├──→ T007–T014 (DB unit tests)
  │
  ▼
T015–T018 (core/status.rs + mod.rs)
  │
  ├──→ T019–T023 (Core unit tests)
  │
  ▼  ← Phase 2 complete ─────────────────────────────────────────────
  │
  ├──→ T024–T032 (US1: cmd_status table + humanize + tests)
  │         │
  │         ▼
  │    T033–T036 (US2: seal_env wiring + tests)
  │         │
  │         ▼
  │    T037–T042 (US3: JSON output + serde structs + tests)
  │         │
  │         ▼
  │    T043–T048 (US4: artifact metadata + tests)
  │
  └──→ T049–T053 (Polish — all independent of each other)
```

## Parallel Opportunities

- **Phase 2**: T007–T014 (DB tests) can all be written in a single pass once T003–T006 are done; T019–T023 (Core tests) can be written in a single pass once T015–T018 are done.
- **Phase 3**: T027–T032 (CLI tests for US1) are all independent functions in the same file.
- **Phase 4**: T034–T036 can be written in a single pass once T033 is done.
- **Phase 5**: T040–T042 (JSON tests) are independent test functions.
- **Phase 6**: T046–T048 (artifact tests) are independent test functions.
- **Phase 7**: T049–T053 are all fully independent of each other.

## Implementation Strategy

**MVP** (minimum releasable increment): Phases 1–3 (T001–T032) — `envy status` renders a table with correct sync labels using directly-inserted DB markers. This delivers US1 (P1) without the encrypt wiring, giving developers immediate visibility even before US2 is done.

**Full feature**: Add Phases 4–7 (T033–T053) — encrypt wiring, JSON output, artifact metadata, and polish.
