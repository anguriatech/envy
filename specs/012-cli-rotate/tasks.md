# Tasks: Envelope Passphrase Rotation

**Input**: Design documents from `/specs/012-cli-rotate/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md

**Tests**: This spec explicitly requests tests (acceptance criteria mention 7 unit tests + 1 E2E scenario). Tests are written FIRST per the project's TDD workflow (Constitution Principle III).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4, US5)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root — matches envy's layout

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization for the new feature

^- [X] T001 Bump Cargo.toml version from 0.2.7 to 0.3.0 in `/home/oriolgv/aaDev/envy-project/envy/Cargo.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented. These tasks establish the contract between the CLI and core layers and the core helper that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

^- [X] T002 Add `pub fn rotate_env` function in `/home/oriolgv/aaDev/envy-project/envy/src/core/sync.rs` (signature: `pub fn rotate_env(vault: &Vault, master_key: &[u8; 32], project_id: &ProjectId, artifact: &mut SyncArtifact, env_name: &str, current_passphrase: &str, new_passphrase: &str) -> Result<(), SyncError>`)
^- [X] T003 [P] Add `Rotate` variant to the `Commands` enum in `/home/oriolgv/aaDev/envy-project/envy/src/cli/mod.rs` with `env: Option<String>` field (matches the shape of `Encrypt`)
^- [X] T004 [P] Add dispatch arm for `Commands::Rotate` in `run()` function in `/home/oriolgv/aaDev/envy-project/envy/src/cli/mod.rs` (calls `commands::cmd_rotate(...)`, maps errors via `format_cli_error` + `cli_exit_code`)
^- [X] T005 [P] Add unit test for `rotate_env` happy path in `/home/oriolgv/aaDev/envy-project/envy/src/core/sync.rs` test module (verifies that a new envelope replaces the old one and other envelopes are byte-identical)
^- [X] T006 [P] Add unit test for `rotate_env` wrong-current-passphrase in `/home/oriolgv/aaDev/envy-project/envy/src/core/sync.rs` test module (verifies `DecryptionFailed` error and the artifact is byte-identical to the pre-rotation state)

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Single-environment passphrase rotation (Priority: P1) 🎯 MVP

**Goal**: A user can rotate the passphrase of a single environment by providing the current and new passphrases interactively. The rotation MUST verify the current passphrase and fail safely (no silent re-seal) on any input error.

**Independent Test**: Seal an envelope with passphrase A, run `envy rotate -e ENV` with passphrase A and a new passphrase B (twice), then attempt to decrypt with A (must fail) and with B (must succeed). The envelope's contents and the vault are unchanged across the rotation.

### Tests for User Story 1 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T007 [P] [US1] Write failing test `rotate_happy_path_seals_then_unseals_with_new` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A → rotate to B → decrypt B succeeds → decrypt A fails)
^- [X] T008 [P] [US1] Write failing test `rotate_wrong_current_passphrase_leaves_artifact_unchanged` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (SHA-256 of `envy.enc` before and after a wrong-pass attempt must be identical)
^- [X] T009 [P] [US1] Write failing test `rotate_new_equals_current_is_rejected` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (error returned, artifact SHA-256 unchanged)
^- [X] T010 [P] [US1] Write failing test `rotate_confirmation_mismatch_is_rejected` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (error returned, artifact SHA-256 unchanged)

### Implementation for User Story 1

^- [X] T011 [US1] Implement `cmd_rotate` function in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` (single-env interactive path: read `env_filter`, resolve current passphrase via `resolve_passphrase_for_env`, resolve new passphrase via `dialoguer::Password::with_confirmation`, wrap both in `Zeroizing<String>`, reject `new == current`, call `crate::core::rotate_env`, print success line with forward-only note)
^- [X] T012 [US1] Verify all US1 tests pass via `cargo test cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently. The MVP (rotate one env interactively) is shippable.

---

## Phase 4: User Story 2 - Rotation fails safely on wrong current passphrase (Priority: P1)

**Goal**: Verified by US1's safety test (T008) and the implementation's reliance on `check_envelope_passphrase` inside `core::sync::rotate_env` (T002). US2 is the **safety invariant** of US1, not a separate code path.

**Independent Test**: The US1 test `rotate_wrong_current_passphrase_leaves_artifact_unchanged` (T008) covers this story's acceptance scenarios.

**Implementation note**: This story is fully covered by US1's implementation (T011) and the `core::sync::rotate_env` helper (T002). No additional tasks are required. The acceptance scenarios are:

- ✅ Scenario 1 (wrong current passphrase → error, exit 2, artifact unchanged): covered by T008 + T011
- ✅ Scenario 2 (Ctrl-C aborts rotation): covered by `dialoguer::Password::interact()` returning `Err` on Ctrl-C, mapped to `CliError::PassphraseInput` (existing exit 2 mapping)
- ✅ Scenario 3 (artifact byte-identical after wrong-passphrase attempt): explicitly verified by T008

**Checkpoint**: US1 + US2 together form the safe-rotation MVP. No new code is required beyond US1.

---

## Phase 5: User Story 3 - Headless rotation in CI / scripted workflows (Priority: P2)

**Goal**: A CI pipeline can rotate a passphrase by setting `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` in the environment. No prompts are displayed. The command exits with 0 on success and 2 on any input-related failure.

**Independent Test**: Run the command with both env vars set, no TTY attached. The rotation must complete without any prompt being displayed and the artifact must be updated.

### Tests for User Story 3 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T013 [P] [US3] Write failing test `rotate_headless_with_env_vars_succeeds` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (sets `ENVY_PASSPHRASE_<ENV>` + `ENVY_PASSPHRASE_<ENV>_NEW`, no TTY, asserts success and SHA-256 change)
^- [X] T014 [P] [US3] Write failing test `rotate_no_tty_no_env_vars_returns_error` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (no env vars, no TTY, asserts `PassphraseInput` error and exit code 2)
^- [X] T015 [P] [US3] Write failing test `rotate_headless_wrong_current_passphrase_returns_error` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (env vars set with wrong current, asserts `PassphraseInput` error and artifact SHA-256 unchanged)
^- [X] T016 [P] [US3] Write failing test `rotate_does_not_honour_global_envy_passphrase` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (only `ENVY_PASSPHRASE` set globally, asserts that rotation does NOT silently use it — must return error)

### Implementation for User Story 3

^- [X] T017 [P] [US3] Add `is_rotate_headless_mode(env_name: &str) -> bool` helper in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` (returns `true` only when BOTH `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` are set and non-empty)
^- [X] T018 [US3] Extend `cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` to support the headless path (when `is_rotate_headless_mode` returns `true`, read both passphrases from env vars, wrap in `Zeroizing<String>`, skip prompts entirely; precedence: when BOTH env vars AND a TTY are present, headless wins per clarification #1)
^- [X] T019 [US3] Verify all US3 tests pass via `cargo test cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, User Stories 1, 2, AND 3 should both work independently — interactive single-env rotation AND headless CI rotation.

---

## Phase 6: User Story 4 - Multi-environment rotation (Priority: P2)

**Goal**: A user can rotate several envelopes in one operation by running `envy rotate` (no `-e` flag) and selecting multiple environments from a MultiSelect prompt. The current/new/confirm prompts are repeated for each selected env.

**Independent Test**: Select multiple environments in the multi-select prompt, enter the passphrases for each, and verify that all selected envelopes are re-sealed and the unselected ones are unchanged.

### Tests for User Story 4 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T020 [P] [US4] Write failing test `rotate_multi_environment_rotates_each` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (two envs in artifact, both re-sealed, SHA-256 of `envy.enc` changes)
^- [X] T021 [P] [US4] Write failing test `rotate_multi_env_one_wrong_skips_others_continue` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (two envs selected, one's current passphrase is wrong — the other rotates successfully, the wrong one is skipped with warning)

### Implementation for User Story 4

^- [X] T022 [US4] Extend `cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` to support the MultiSelect path (when `env_filter` is `None`, read the artifact via `crate::core::read_artifact` to get the env list, present a `dialoguer::MultiSelect`, iterate selected envs in a stable order)
^- [X] T023 [US4] Extend the loop in `cmd_rotate` to print per-env success/warning lines for multi-env mode (in the order they were processed, one block per env)
^- [X] T024 [US4] Add the atomic write call to `cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` (after the per-env loop, call `crate::core::write_artifact_atomic(&artifact, artifact_path)`)
^- [X] T025 [US4] Verify all US4 tests pass via `cargo test cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, User Stories 1, 2, 3, AND 4 should all work independently.

---

## Phase 7: User Story 5 - Empty-envelope guard (Priority: P3)

**Goal**: An attempt to rotate an environment whose local vault contains zero secrets prints a warning and skips the rotation. The artifact is unchanged.

**Independent Test**: Run the rotation against an environment with no secrets in the local vault and verify that the envelope in `envy.enc` is unchanged and a warning is printed.

### Tests for User Story 5 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T026 [P] [US5] Write failing test `rotate_empty_env_skips_with_warning` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (env in artifact but 0 secrets in vault, asserts warning printed and artifact SHA-256 unchanged)

### Implementation for User Story 5

^- [X] T027 [US5] Add empty-env guard to the per-env loop in `cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` (mirror `cmd_encrypt` pattern at `src/cli/commands.rs:727-736` — call `crate::core::list_secret_keys`, if empty, print yellow ⚠ warning and `continue`)
^- [X] T028 [US5] Verify the US5 test passes via `cargo test cmd_rotate` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: All user stories should now be independently functional. US5 is a guard rail that prevents a meaningless `envy.enc` change.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, E2E coverage, and final quality gate validation across all stories.

^- [X] T029 [P] Add E2E Scenario 10 "Envelope Passphrase Rotation" to `/home/oriolgv/aaDev/envy-project/envy/tests/e2e_devops_scenarios.sh` (init → set → encrypt with A → rotate A→B headless → encrypt with B → assert success; then wrong-current-pass attempt → assert exit 2 + SHA-256 unchanged)
^- [X] T030 [P] Add `envy rotate [-e ENV]` to the command table in `/home/oriolgv/aaDev/envy-project/envy/README.md`
^- [X] T031 [P] Add a paragraph about `envy rotate` to the "Multi-Environment with Separate Passphrases" section in `/home/oriolgv/aaDev/envy-project/envy/README.md` (explain that rotation is the safe path for key rotation vs `envy encrypt`'s silent headless rotation)
^- [X] T032 [P] Add `envy rotate` mention to the GitOps section in `/home/oriolgv/aaDev/envy-project/envy/docs/developer-guide.md` (note that it re-seals via `seal_env`, no new crypto primitives)
^- [X] T033 [P] Add `envy rotate` mention to the cryptographic flow section in `/home/oriolgv/aaDev/envy-project/envy/docs/developer-guide.md` (Argon2id KDF + AES-256-GCM unchanged, fresh salt + nonce on each rotation)
^- [X] T034 Run `cargo fmt --check` from `/home/oriolgv/aaDev/envy-project/envy/` and fix any formatting issues
^- [X] T035 Run `cargo clippy -- -D warnings` from `/home/oriolgv/aaDev/envy-project/envy/` and resolve all lints
^- [X] T036 Run `cargo test` from `/home/oriolgv/aaDev/envy-project/envy/` and verify all tests pass (unit + integration)
^- [X] T037 Run `cargo build` from `/home/oriolgv/aaDev/envy-project/envy/` and verify the binary builds clean
^- [X] T038 Run `ENVY_BIN=$(pwd)/target/debug/envy bash tests/e2e_devops_scenarios.sh` from `/home/oriolgv/aaDev/envy-project/envy/` and verify all 10 E2E scenarios pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-7)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P1)**: No new tasks - fully covered by US1's safety test (T008) and the `core::sync::rotate_env` helper (T002)
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - Independent of US1/US2 (adds headless path, separate test cases)
- **User Story 4 (P2)**: Depends on US1 (extends the cmd_rotate loop) - reuses the single-env implementation
- **User Story 5 (P3)**: Can start after US1's implementation is in place (adds a guard at the top of the per-env loop) - Independent of US3/US4

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Tests within a story marked [P] can be written in parallel
- Implementation depends on the test for that story being written first
- Story complete before moving to next priority

### Parallel Opportunities

- All Foundational tasks marked [P] (T003, T004, T005, T006) can run in parallel
- All tests for a user story marked [P] can be written in parallel
- Documentation tasks at the end (T030, T031, T032, T033) can run in parallel
- Polish-phase verification tasks (T034, T035, T036, T037, T038) are sequential (each depends on the previous passing)

---

## Parallel Example: User Story 1

```bash
# Launch all 4 tests for User Story 1 together (test-writing is parallel):
Task: "Write failing test rotate_happy_path_seals_then_unseals_with_new in src/cli/commands.rs"
Task: "Write failing test rotate_wrong_current_passphrase_leaves_artifact_unchanged in src/cli/commands.rs"
Task: "Write failing test rotate_new_equals_current_is_rejected in src/cli/commands.rs"
Task: "Write failing test rotate_confirmation_mismatch_is_rejected in src/cli/commands.rs"

# After all tests fail (TDD red phase), implement cmd_rotate (green phase):
Task: "Implement cmd_rotate in src/cli/commands.rs (single-env interactive path)"

# Verify all tests pass:
Task: "Verify all US1 tests pass via cargo test cmd_rotate"
```

## Parallel Example: Foundational Phase

```bash
# These 4 tasks can run in parallel — different files, no dependencies:
Task: "Add pub fn rotate_env in src/core/sync.rs"          # T002
Task: "Add Rotate variant to Commands enum in src/cli/mod.rs"  # T003
Task: "Add dispatch arm in run() in src/cli/mod.rs"            # T004
Task: "Add unit test for rotate_env in src/core/sync.rs"       # T005

# T006 (the second sync.rs test) can also run in parallel with T005 — both are
# in the same file but are independent test functions.
```

## Parallel Example: Polish Phase

```bash
# Documentation tasks are independent files — run in parallel:
Task: "Add envy rotate to command table in README.md"        # T030
Task: "Add rotation paragraph in README.md"                  # T031
Task: "Add envy rotate to GitOps section in developer-guide.md"  # T032
Task: "Add envy rotate to crypto flow section in developer-guide.md"  # T033
```

---

## Implementation Strategy

### MVP First (User Story 1 + User Story 2)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002-T006)
3. Complete Phase 3: User Story 1 (T007-T012)
4. User Story 2 is automatically covered by US1's safety test (T008)
5. **STOP and VALIDATE**: Run `cargo test cmd_rotate` and the existing E2E suite; both should pass
6. Deploy/demo if ready — single-env interactive rotation is shippable as a 0.3.0 release

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add US1 + US2 → Test independently → Deploy/Demo (MVP!)
3. Add US3 (headless) → Test independently → Deploy/Demo (CI workflows now supported)
4. Add US4 (multi-env) → Test independently → Deploy/Demo (multi-env workflows now supported)
5. Add US5 (empty-env guard) → Test independently → Deploy/Demo (guard rail added)
6. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers (envy is typically solo, but if a team is on it):

1. Team completes Setup + Foundational together (T001-T006)
2. Once Foundational is done:
   - Developer A: User Story 1 (T007-T012) — the MVP
   - Developer B: User Story 3 (T013-T019) — headless CI
   - Developer C: User Story 5 (T026-T028) — empty-env guard (small, can be done anytime after US1)
3. Developer A finishes US1 first, then picks up US4 (T020-T025) — multi-env
4. Stories complete and integrate independently; no merge conflicts because each story edits `cmd_rotate` in different parts (US1 is the loop body, US4 is the env-selection prelude, US5 is a guard at the top of the loop)

---

## Notes

- [P] tasks = different files, no dependencies — these can be parallelized
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- US2 is a "verified by US1" story — no new tasks, no new code
- Verify tests fail before implementing (TDD red-green-refactor)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same-file conflicts (multiple US tasks editing `cmd_rotate` should be ordered), cross-story dependencies that break independence
- Memory hygiene invariant (clarification #3): every passphrase binding in `cmd_rotate` MUST be `Zeroizing<String>` and MUST be dropped before any early `return`
- 4-layer invariant (Constitution Principle IV): `cmd_rotate` MUST NOT call `crypto::seal_envelope` directly; it goes through `core::sync::rotate_env`
- No-new-error-variants invariant: every error from `cmd_rotate` maps to an existing `CliError` variant (`PassphraseInput` exit 2, `EnvNotFound` exit 3, `FileNotFound` exit 1, `VaultOpen` exit 4)
