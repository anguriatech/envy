# Tasks: Multi-Environment Encryption and Smart Merging

**Input**: Design documents from `specs/009-multi-env-encrypt/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, contracts/encrypt-command.md ✓, quickstart.md ✓

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- All file paths are relative to the repository root

---

## Phase 1: Setup

**Purpose**: Project initialization — add new dependency and data asset before any code is written.

- [x] T001 Add `rand = "0.8"` to `[dependencies]` in `Cargo.toml` (Research Decision 2 — needed for `SliceRandom::choose` without modulo bias)
- [x] T002 [P] Create `data/eff-wordlist.txt` — download and add the EFF Large Wordlist (7776 lines, format: `DDDDD\tword` per line) to the repository root `data/` directory (Research Decision 1)
- [x] T003 [P] Create empty `src/crypto/diceware.rs` file and declare `pub mod diceware;` in `src/crypto/mod.rs`; add `pub use diceware::suggest_passphrase;` to the re-exports in `src/crypto/mod.rs`

**Checkpoint**: `cargo check` must pass before Phase 2.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core primitives that ALL user story implementations depend on. Must be complete before any story phase begins.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete and `cargo test` passes.

- [x] T004 Implement `suggest_passphrase(word_count: usize) -> String` in `src/crypto/diceware.rs`: embed `data/eff-wordlist.txt` via `include_str!("../../data/eff-wordlist.txt")`, parse words via `OnceLock`, use `rand::rngs::OsRng` + `rand::seq::SliceRandom::choose` (FR-010; Research Decisions 1, 2)
- [x] T005 Add unit tests for `suggest_passphrase` inside `src/crypto/diceware.rs`: `wordlist_has_7776_entries`, `suggest_4_words_has_3_spaces`, `suggest_is_non_empty`, `two_suggestions_differ` (Research Decision 1 — probabilistic collision test)
- [x] T006 Add `pub fn seal_env(vault, master_key, project_id, env_name, passphrase) -> Result<EncryptedEnvelope, SyncError>` to `src/core/sync.rs`: reads secrets for one env via `crate::core::get_env_secrets`, calls `seal_envelope`, returns the envelope (plan.md §2.1; FR-001, FR-002)
- [x] T007 Add `pub fn write_artifact_atomic(artifact, path) -> Result<(), SyncError>` to `src/core/sync.rs`: writes JSON to `envy.enc.tmp` (sibling path), then `std::fs::rename` to `path`; update existing `write_artifact` to delegate to it (plan.md §2.2; FR-006, SC-003)
- [x] T008 Add `pub fn check_envelope_passphrase(passphrase, env_name, envelope) -> bool` to `src/core/sync.rs`: wraps `unseal_envelope(...).is_ok()` (plan.md §2.3; FR-008 — pre-flight check)
- [x] T009 Re-export `seal_env`, `write_artifact_atomic`, and `check_envelope_passphrase` from `src/core/mod.rs` alongside the existing sync re-exports
- [x] T010 Add unit test for `write_artifact_atomic` in `src/core/sync.rs`: write an artifact, verify `envy.enc` exists and matches; verify the `.tmp` file does NOT exist after success (FR-006)
- [x] T011 Add unit test for `check_envelope_passphrase` in `src/core/sync.rs`: seal an envelope with `"pass-A"`, assert `check_envelope_passphrase("pass-A", ...)` returns `true` and `check_envelope_passphrase("pass-B", ...)` returns `false` (FR-008)

**Checkpoint**: `cargo test` must pass (all new tests green) before Phase 3.

---

## Phase 3: User Story 1 — CI/CD Headless Encryption (Priority: P1) 🎯 MVP

**Goal**: `envy encrypt` works headlessly using `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE` env vars, zero prompts.

**Independent Test**: Set `ENVY_PASSPHRASE_PRODUCTION=secret`, run `envy encrypt` without a TTY, assert exit 0 and `envy.enc` contains the `production` envelope.

- [x] T012 [US1] Add `fn resolve_passphrase_for_env(env_name: &str, confirm: bool, suggested: Option<&str>) -> Result<Option<Zeroizing<String>>, CliError>` to `src/cli/commands.rs`: implement the 3-tier priority (env-specific var → `ENVY_PASSPHRASE` → interactive prompt; returns `Ok(None)` only when no env var and no TTY); normalise env name via `.to_uppercase().replace('-', "_")` (plan.md §3.1; FR-001, FR-002, FR-003, FR-012)
- [x] T013 [US1] Add `fn is_headless_mode(env_names: &[String]) -> bool` to `src/cli/commands.rs`: returns true if `ENVY_PASSPHRASE` or any `ENVY_PASSPHRASE_<ENV>` is set and non-whitespace (plan.md §3.2; FR-004)
- [x] T014 [US1] Rewrite `cmd_encrypt` in `src/cli/commands.rs` — Phase 1 of 3 (headless path only): (1) list all env names from vault, (2) if `is_headless_mode` → iterate all envs, call `resolve_passphrase_for_env`, skip envs returning `Ok(None)`, seal each via `core::seal_env`, merge into existing artifact (read via `core::read_artifact` or start empty on `FileNotFound`, abort on parse error per FR-013), write via `core::write_artifact_atomic` (plan.md §3.3; FR-001–FR-006, FR-012, FR-013)
- [x] T015 [US1] Update existing `cmd_encrypt` tests in `src/cli/commands.rs` to match the new signature and behaviour; add test for `ENVY_PASSPHRASE_<ENV>` resolution (e.g., `ENVY_PASSPHRASE_DEVELOPMENT=...` seals only `development`) (FR-001, FR-012)
- [x] T016 [US1] Add test for smart merge in `src/cli/commands.rs`: pre-populate `envy.enc` with a `production` envelope, then run headless encrypt for `development` only, assert both envelopes present and `production` bytes unchanged (FR-005, SC-002)

**Checkpoint**: Headless mode fully functional. `cargo test` passes.

---

## Phase 4: User Story 2 — Smart Merge with Atomic Writes (Priority: P2)

**Goal**: `envy.enc` always contains the union of existing + newly sealed envelopes; writes are atomic.

**Independent Test**: Pre-populate `envy.enc` with a `production` envelope, encrypt `development` only, assert both present and `production` unchanged. Verify `.tmp` file is absent after success.

*Note*: The core of smart merge (read existing + merge + atomic write) is already implemented in T014. This phase adds the `-e` flag wiring and malformed-JSON abort path.

- [x] T017 [US2] Wire `env_filter: Option<&str>` parameter of `cmd_encrypt` to the selected-envs logic: when a `-e` flag is provided headlessly, seal ONLY that environment (not all) — ensures `-e` still works after the rewrite (plan.md §3.3 step 3; FR-004, FR-005)
- [x] T018 [US2] Add test in `src/cli/commands.rs`: write invalid JSON to `envy.enc`, run `cmd_encrypt`, assert it returns an error (not a successful overwrite) — verifies FR-013 malformed-artifact abort
- [x] T019 [US2] Add test in `src/cli/commands.rs`: kill-signal simulation via `write_artifact_atomic` — write a valid `envy.enc`, then write a valid `.tmp` file directly, verify that after a successful `cmd_encrypt` run the `.tmp` is gone and `envy.enc` is correct (atomic write guarantee, FR-006)

**Checkpoint**: `cargo test` passes. Smart merge and atomic write verified.

---

## Phase 5: User Story 3 — Interactive Multi-Environment Selection (Priority: P3)

**Goal**: Running `envy encrypt` with a TTY and no env vars shows a `MultiSelect` menu of all vault environments.

**Independent Test**: Run `cmd_encrypt` with a mocked TTY selecting two of three environments; assert only those two are updated in `envy.enc`.

- [x] T020 [US3] Add the interactive branch to `cmd_encrypt` in `src/cli/commands.rs`: when `!is_headless_mode` and no `env_filter` → use `dialoguer::MultiSelect` to show all vault environments; if zero selected → print `"Nothing to encrypt."` and return `Ok(())` (plan.md §3.3 steps 3–4; FR-007, FR-011)
- [x] T021 [US3] Add the zero-environments guard to `cmd_encrypt` in `src/cli/commands.rs`: if `vault.list_environments(project_id)` returns empty → print `"No environments found. Use 'envy set' to add secrets first."` and return `Ok(())` (FR-007, spec Edge Cases)
- [x] T022 [US3] Add the interactive passphrase prompt path to `cmd_encrypt` in `src/cli/commands.rs` for existing envelopes (no Diceware, no double-entry): calls `resolve_passphrase_for_env(env_name, false, None)` (plan.md §3.3 step 6c; FR-009 — new envs handled in US5)

**Checkpoint**: Interactive selection path compiles and is manually testable with a TTY.

---

## Phase 6: User Story 4 — Key-Rotation Protection (Priority: P4)

**Goal**: When re-encrypting an existing envelope with a different passphrase, a rotation warning is shown and the operation defaults to abort unless explicitly confirmed.

**Independent Test**: Seal `development` with passphrase A, attempt to re-encrypt with passphrase B, assert the rotation warning fires and the operation is aborted on default (Enter/N).

- [x] T023 [US4] Add `fn confirm_key_rotation(env_name: &str) -> Result<bool, CliError>` to `src/cli/commands.rs`: prints the rotation warning message, uses `dialoguer::Confirm::new().default(false)` to prompt (plan.md §3.5; FR-008)
- [x] T024 [US4] Wire the pre-flight check into `cmd_encrypt` in `src/cli/commands.rs` (interactive path only): for each existing envelope in `envy.enc`, call `core::check_envelope_passphrase`; if `false` → call `confirm_key_rotation`; if user says No → skip this env (continue loop); if Yes → proceed to seal (plan.md §3.3 step 6d; FR-008, SC-004)
- [x] T025 [US4] Add test in `src/cli/commands.rs` for the pre-flight check: seal `development` with `"pass-A"`, then call `check_envelope_passphrase("pass-B", ...)`, assert it returns `false` and the rotation warning path would be triggered (FR-008)

**Checkpoint**: `cargo test` passes. Pre-flight check logic verified.

---

## Phase 7: User Story 5 — Diceware Passphrase Suggestion (Priority: P5)

**Goal**: When encrypting a new environment interactively, a Diceware passphrase is suggested; if accepted, a "SAVE THIS NOW" banner is displayed.

**Independent Test**: Call `cmd_encrypt` for a new environment (not in `envy.enc`) interactively, verify a suggestion is shown and that accepting it causes the banner to print to stderr.

- [x] T026 [US5] Add `fn print_diceware_banner(passphrase: &str)` to `src/cli/commands.rs`: prints a high-visibility yellow/bold banner with the passphrase and the message "You will not be shown this passphrase again." to `stderr` (plan.md §3.4; FR-010, SC-005)
- [x] T027 [US5] Wire Diceware suggestion into `cmd_encrypt` for new environments (interactive path): call `crate::crypto::suggest_passphrase(4)`, pass as `suggested` to `resolve_passphrase_for_env(env_name, true, Some(&suggestion))`; if the user accepted the suggestion (returned passphrase equals suggestion) → call `print_diceware_banner` (plan.md §3.3 step 6c; FR-009, FR-010)
- [x] T028 [US5] Update `resolve_passphrase_for_env` in `src/cli/commands.rs` to handle the `suggested` parameter: when `suggested.is_some()`, display it in the prompt text and treat an empty interactive input as "accept suggestion" — return the suggestion string in that case (plan.md §3.1; FR-010)

**Checkpoint**: Full interactive flow compiles and is manually testable end-to-end.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [x] T029 [P] Update `cmd_encrypt` success output in `src/cli/commands.rs` to list ALL updated environments (not just the current run's sealed count) — print `"Sealed N environment(s) → <path>"` where N is the number of envelopes updated in this run, followed by `"  ✓  <env_name>"` per updated env
- [x] T030 [P] Run `cargo clippy -- -D warnings` and fix any new warnings introduced by this feature across all modified files (`src/crypto/diceware.rs`, `src/crypto/mod.rs`, `src/core/sync.rs`, `src/core/mod.rs`, `src/cli/commands.rs`, `Cargo.toml`)
- [x] T031 [P] Run `cargo fmt` on all modified files and commit formatting changes
- [x] T032 [P] Run `cargo audit` to confirm no new CVEs introduced by `rand = "0.8"` direct dep — 2 existing warnings from transitive deps (zbus/fastrand), none from rand
- [x] T033 Add E2E test scenario to `tests/e2e_devops_scenarios.sh`: headless multi-env scenario using `ENVY_PASSPHRASE_DEVELOPMENT` + `ENVY_PASSPHRASE_PRODUCTION`, assert both envelopes in `envy.enc` after encrypt, assert smart merge preserves a third pre-existing envelope (SC-002, FR-005)

---

## Phase 9: QA Fast-Follow (F1 + F2)

- [x] T034 [F1] Add empty-env guard to `cmd_encrypt` in `src/cli/commands.rs`: before sealing, call `crate::core::list_secret_keys`; if count is 0 → print `"  ⚠  environment '<env>' has 0 secrets, skipping"` to stderr and `continue` the loop (QA-F1)
- [x] T035 [F1] Add unit test `encrypt_skips_empty_env_with_warning` in `src/cli/commands.rs`: set `ENVY_PASSPHRASE`, call `cmd_encrypt` for an env with 0 secrets, assert `Ok(())` and that `envy.enc` does NOT contain that environment (QA-F1)
- [x] T036 [F2] Add `pub fn unseal_env(artifact, env_name, passphrase) -> Result<Option<BTreeMap<String, Zeroizing<String>>>, SyncError>` to `src/core/sync.rs`: returns `Ok(Some(secrets))` on success, `Ok(None)` on wrong passphrase / env not found; re-export from `src/core/mod.rs` (QA-F2)
- [x] T037 [F2] Update `cmd_decrypt` in `src/cli/commands.rs`: detect headless mode from artifact env names; if headless → per-env passphrase resolution loop using `resolve_passphrase_for_env` + `crate::core::unseal_env`; else → existing single-passphrase flow unchanged (QA-F2)
- [x] T038 [F2] Add unit test `decrypt_uses_per_env_passphrase_var` in `src/cli/commands.rs`: seal `development` with `"dev-pass"`, set `ENVY_PASSPHRASE_DEVELOPMENT="dev-pass"`, run `cmd_decrypt`, assert secrets are imported (QA-F2)
- [x] T039 [P] Run `cargo clippy -- -D warnings`, `cargo fmt`, `cargo test` and confirm all pass

---

## Dependency Graph

```
T001 (Cargo.toml) ──┐
T002 (wordlist)  ──┤
T003 (mod.rs)    ──┤
                    ▼
T004 (diceware.rs) → T005 (diceware tests)
T006 (seal_env)   ─┐
T007 (write_atomic)─┤→ T009 (re-exports) → T010, T011
T008 (preflight)  ─┘
                    ▼
     T012, T013, T014 (headless core) → T015, T016
                    ▼
             T017, T018, T019 (smart merge)
                    ▼
             T020, T021, T022 (interactive select)
                    ▼
             T023, T024, T025 (rotation protection)
                    ▼
             T026, T027, T028 (Diceware UX)
                    ▼
         T029–T033 (polish, in parallel)
```

## Parallel Opportunities

- **Phase 1**: T002 (wordlist download) and T003 (mod.rs scaffolding) can run in parallel after T001.
- **Phase 2**: T004–T005 (Diceware) are independent of T006–T011 (Core) and can run in parallel.
- **Phase 8**: T029–T033 are all independent and can run in parallel.

## Implementation Strategy

**MVP** (minimum releasable increment): Phases 1–3 (T001–T016) — headless CI mode with smart merge and atomic writes. This unblocks all CI pipelines (US1, P1) and eliminates the data-loss risk in team collaboration (US2, P2) without requiring any interactive UX changes.

**Full feature**: Add Phases 4–7 (T017–T028) — interactive selection, key-rotation protection, Diceware suggestion.
