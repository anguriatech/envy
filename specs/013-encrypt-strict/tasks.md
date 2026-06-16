# Tasks: Strict `envy encrypt` (No Silent Key Rotation)

**Input**: Design documents from `/specs/013-encrypt-strict/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md

**Tests**: This spec explicitly requests tests (acceptance criteria mention 9 new unit tests + 1 existing test updated + 1 new E2E scenario). Tests are written FIRST per the project's TDD workflow (Constitution Principle III).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4, US5)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root — matches envy's layout

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization for the new feature

^- [X] T001 Bump Cargo.toml version from 0.3.0 to 0.3.1 in `/home/oriolgv/aaDev/envy-project/envy/Cargo.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented. These tasks establish the production-code change that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

^- [X] T002 [P] Delete the `confirm_key_rotation` function (lines 551-570) and its 6-line doc comment in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` (function definition + doc comment + closing brace; verify with `grep -r confirm_key_rotation src/` returning zero matches — SC-006)
^- [X] T003 [P] Remove the `dialoguer::Confirm` import from `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` if no other code in the file uses it (verify with `grep -n "Confirm::" src/cli/commands.rs` — should return zero matches after this task)
^- [X] T004 Replace the `if !headless { ... confirm_key_rotation ... }` block (lines 748-763) in `cmd_encrypt` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` with an unconditional strict-verify-or-fail block that calls `crate::core::check_envelope_passphrase` and returns `Err(CliError::PassphraseInput(format!("passphrase does not match the existing envelope.\nhint: use `envy rotate -e ENV` to change the envelope's passphrase.")))` on mismatch
^- [X] T005 Update the assertion comment and message at line 2084-2093 in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module to remove the "key-rotation warning path" reference (test logic is unchanged — the helper `check_envelope_passphrase` is still correct)

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - First-time seal of a new envelope (Priority: P1)

**Goal**: A developer can run `envy encrypt -e ENV` in a fresh project and be prompted for a new passphrase (with confirmation in interactive mode) or use the env var (in headless mode). The envelope is created. The first-time-seal path is unchanged from v0.3.0 — this phase verifies that.

**Independent Test**: Seal an envelope with passphrase A in a fresh project; verify the envelope is created; verify the CLI prints a success message; verify `envy decrypt` with A returns the secrets.

### Tests for User Story 1 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T006 [P] [US1] Write/verify test `encrypt_first_time_seal_interactive_succeeds` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (pre-existing behaviour: seal A in fresh project succeeds)
^- [X] T007 [P] [US1] Write/verify test `encrypt_first_time_seal_headless_succeeds` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (pre-existing behaviour: seal A via `ENVY_PASSPHRASE_<ENV>` succeeds)

### Implementation for User Story 1

^- [X] T008 [US1] Verify all US1 tests pass via `cargo test encrypt_first_time` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, US1 confirms that the first-time-seal path is unchanged.

---

## Phase 4: User Story 2 - Update seal with a matching passphrase (Priority: P1)

**Goal**: A developer can run `envy encrypt -e ENV` in a project with an existing envelope and the correct passphrase. The envelope is re-sealed with the same passphrase (fresh salt + nonce are generated because `seal_envelope` is called fresh). The matching-passphrase update case is unchanged from v0.3.0 — this phase verifies and adds explicit tests.

**Independent Test**: Seal an envelope with passphrase A, change a local secret, re-seal with A. The envelope is re-created (byte-different from the old one — new salt, new nonce) but decrypts with the same passphrase.

### Tests for User Story 2 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T009 [P] [US2] Write failing test `encrypt_update_seal_matching_passphrase_succeeds` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A, then re-seal with A — succeeds, exit 0)
^- [X] T010 [P] [US2] Write failing test `encrypt_update_seal_headless_matching_succeeds_and_byte_changes` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A headlessly, then re-seal with A headlessly — succeeds, SHA-256 of envy.enc changes because fresh salt + nonce)

### Implementation for User Story 2

^- [X] T011 [US2] Verify all US2 tests pass via `cargo test encrypt_update_seal_matching` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, US2 confirms that the matching-passphrase update path is unchanged.

---

## Phase 5: User Story 3 - Mismatch with existing envelope fails clearly (Priority: P1)

**Goal**: A developer running `envy encrypt -e ENV` with a passphrase that does NOT match the existing envelope sees a clear error message and the CLI exits 2. The artifact is byte-identical to its pre-attempt state. This is the **safety invariant** that the spec exists to enforce — and the breaking change vs v0.3.0.

**Independent Test**: Seal with A, then run `envy encrypt -e ENV` with B (interactive AND headless AND global `ENVY_PASSPHRASE`). All three variants: exit 2, error message contains "passphrase does not match" + "envy rotate", SHA-256 of envy.enc is unchanged.

### Tests for User Story 3 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T012 [P] [US3] Write failing test `encrypt_update_seal_mismatch_interactive_fails_exit_2` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A, then re-seal with B in interactive mode — returns `Err(CliError::PassphraseInput(_))`, exit 2)
^- [X] T013 [P] [US3] Write failing test `encrypt_update_seal_mismatch_headless_fails_exit_2` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A, then re-seal with B in headless mode — returns `Err(CliError::PassphraseInput(_))`, exit 2; the headless path is the breaking change)
^- [X] T014 [P] [US3] Write failing test `encrypt_update_seal_global_envy_passphrase_mismatch_fails` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A, then re-seal with `ENVY_PASSPHRASE=WRONG` (global) — returns `Err(CliError::PassphraseInput(_))`, exit 2)
^- [X] T015 [P] [US3] Write failing test `encrypt_mismatch_leaves_artifact_unchanged_sha256` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A, then attempt re-seal with B — SHA-256 of envy.enc is byte-identical before and after the failed attempt; SC-003 acceptance criterion)

### Implementation for User Story 3

^- [X] T016 [US3] Verify all US3 tests pass via `cargo test encrypt_update_seal_mismatch` and `cargo test encrypt_mismatch` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: At this point, US3 is the **MVP** of this spec — the safety invariant is enforced. The breaking change is in place.

---

## Phase 6: User Story 4 - Empty-vault guard (unchanged from today) (Priority: P2)

**Goal**: A developer running `envy encrypt -e empty-env` when the local vault has zero secrets for that environment sees a yellow warning and the CLI exits 0. The artifact is not modified. The existing behaviour is preserved.

**Independent Test**: Create an env row in the vault with zero secrets, then run `envy encrypt -e empty-env`. The CLI prints a warning and exits 0; envy.enc is unchanged.

### Tests for User Story 4 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T017 [P] [US4] Verify pre-existing test `encrypt_skips_empty_env_with_warning` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (line 2087) still passes (no new test needed; verify the existing one is not broken by the T004 change)

### Implementation for User Story 4

^- [X] T018 [US4] Verify US4 test still passes via `cargo test encrypt_skips_empty_env` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: US4 confirms that the existing empty-vault guard is preserved.

---

## Phase 7: User Story 5 - First-time seal with empty vault consistency (Priority: P2)

**Goal**: A developer running `envy encrypt -e ENV` against an existing envelope when the local vault has zero secrets for that env (e.g. they deleted the only secret) sees the empty-vault warning and exits 0. The artifact is NOT overwritten with an empty envelope. This is the new consistency rule: empty-vault applies in BOTH new-env and update-env cases.

**Independent Test**: Seal an envelope with passphrase A and one secret. Locally, delete that secret. Run `envy encrypt -e ENV` with A. The CLI prints the empty-vault warning and exits 0; the artifact is unchanged; `envy decrypt` with A still returns the original secret.

### Tests for User Story 5 ⚠️ Write these FIRST, ensure they FAIL before implementation

^- [X] T019 [P] [US5] Write failing test `encrypt_update_seal_empty_vault_skips_with_warning` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (seal A with one secret, delete the secret locally, attempt re-seal with A — returns `Ok(())`, exit 0, warning printed, SHA-256 of envy.enc is unchanged)

### Implementation for User Story 5

^- [X] T020 [US5] Verify all US5 tests pass via `cargo test encrypt_update_seal_empty_vault` in `/home/oriolgv/aaDev/envy-project/envy/`

**Checkpoint**: US5 confirms that the empty-vault guard now applies in BOTH new-env and update-env cases.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, E2E coverage, and final quality gate validation across all stories.

^- [X] T021 [P] Add E2E Scenario 11 "Strict `envy encrypt`" to `/home/oriolgv/aaDev/envy-project/envy/tests/e2e_devops_scenarios.sh` (init → set → seal A headless → attempt seal with B headless → assert exit 2 + SHA-256 unchanged + error message contains "passphrase does not match" and "envy rotate")
^- [X] T022 [P] Update the `envy encrypt` row in the command table in `/home/oriolgv/aaDev/envy-project/envy/README.md` (line ~309) to mention the strict behaviour: "Seal vault into `envy.enc` (strict: passphrase must match an existing envelope — use `envy rotate` to change it)"
^- [X] T023 [P] Add a "Strict `envy encrypt`" subsection to the "Multi-Environment with Separate Passphrases" section in `/home/oriolgv/aaDev/envy-project/envy/README.md` (line ~362) explaining the strict behaviour + the `envy rotate` escape hatch
^- [X] T024 [P] Add a "Strict `envy encrypt` since v0.3.1" paragraph to the GitOps section in `/home/oriolgv/aaDev/envy-project/envy/docs/developer-guide.md` (line ~76) — explains the three-case contract, closes the silent-rotation gap, points to `envy rotate`
^- [X] T025 [P] Run `cargo fmt --check` from `/home/oriolgv/aaDev/envy-project/envy/` and fix any formatting issues
^- [X] T026 [P] Run `cargo clippy -- -D warnings` from `/home/oriolgv/aaDev/envy-project/envy/` and resolve all lints
^- [X] T027 [P] Run `cargo test` from `/home/oriolgv/aaDev/envy-project/envy/` and verify all tests pass (unit + integration + the 9 new rotate-adjacent tests)
^- [X] T028 [P] Run `cargo build` from `/home/oriolgv/aaDev/envy-project/envy/` and verify the binary builds clean
^- [X] T029 [P] Run `ENVY_BIN=$(pwd)/target/debug/envy bash tests/e2e_devops_scenarios.sh` from `/home/oriolgv/aaDev/envy-project/envy/` and verify all 11 E2E scenarios pass
^- [X] T030 [P] Run `grep -r confirm_key_rotation src/` from `/home/oriolgv/aaDev/envy-project/envy/` and verify zero matches (SC-006 hard acceptance criterion)

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
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 3 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 4 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 5 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories

### Within Each User Story

- Tests MUST be written and FAIL before implementation (TDD red-green-refactor)
- Tests within a story marked [P] can be written in parallel
- Implementation depends on the test for that story being written first
- Story complete before moving to next priority

### Parallel Opportunities

- All Foundational tasks marked [P] (T002, T003) can run in parallel (different concerns; both touch `src/cli/commands.rs` but in different sections)
- All tests for a user story marked [P] can be written in parallel
- Documentation tasks at the end (T022, T023, T024) can run in parallel
- Polish-phase verification tasks (T025, T026, T027, T028, T029, T030) are parallel (different commands, no dependencies)

---

## Parallel Example: Phase 5 (User Story 3 - Mismatch)

```bash
# Launch all 4 tests for User Story 3 together (test-writing is parallel):
Task: "Write failing test encrypt_update_seal_mismatch_interactive_fails_exit_2 in src/cli/commands.rs"
Task: "Write failing test encrypt_update_seal_mismatch_headless_fails_exit_2 in src/cli/commands.rs"
Task: "Write failing test encrypt_update_seal_global_envy_passphrase_mismatch_fails in src/cli/commands.rs"
Task: "Write failing test encrypt_mismatch_leaves_artifact_unchanged_sha256 in src/cli/commands.rs"

# Verify all tests pass:
Task: "Verify all US3 tests pass via cargo test encrypt_update_seal_mismatch"
```

## Parallel Example: Polish Phase

```bash
# Documentation tasks are independent files — run in parallel:
Task: "Update envy encrypt row in command table in README.md"
Task: "Add Strict envy encrypt subsection in README.md"
Task: "Add Strict envy encrypt paragraph in docs/developer-guide.md"

# Verification tasks are independent commands — run in parallel:
Task: "Run cargo fmt --check"
Task: "Run cargo clippy -- -D warnings"
Task: "Run cargo test"
Task: "Run cargo build"
Task: "Run E2E suite"
Task: "Run grep -r confirm_key_rotation src/"
```

---

## Implementation Strategy

### MVP First (User Story 3 — the breaking change)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002-T005) — the production code change
3. Complete Phase 5: User Story 3 (T012-T016) — the tests for the breaking change
4. **STOP and VALIDATE**: Run `cargo test encrypt_update_seal_mismatch` — the 4 tests must pass
5. Deploy/demo if ready — the breaking change is shippable as 0.3.1 once US3 is green

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready (production code change is in place)
2. Add US1 → Test independently → confirms first-time-seal path is unchanged
3. Add US2 → Test independently → confirms matching-passphrase update is unchanged
4. Add US3 → Test independently → enforces the safety invariant (MVP)
5. Add US4 → Test independently → confirms existing empty-vault guard is preserved
6. Add US5 → Test independently → tightens empty-vault guard for update case
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001-T005) — the single-file change
2. Once Foundational is done:
   - Developer A: User Story 3 (T012-T016) — the critical breaking change (P1)
   - Developer B: User Story 1 (T006-T008) — confirms first-time-seal is unchanged (P1)
   - Developer C: User Story 2 (T009-T011) — confirms matching update is unchanged (P1)
3. After P1 stories are green:
   - Developer A: User Story 4 (T017-T018) — preserves existing empty-vault guard
   - Developer B: User Story 5 (T019-T020) — tightens empty-vault guard for update
4. Polish phase (T021-T030) can be split across the team

---

## Notes

- [P] tasks = different files, no dependencies — these can be parallelized
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- US3 is the **MVP** — it is the breaking change that the spec exists to enforce
- US1, US2, US4 are **regression** stories — they verify that the existing happy paths are NOT broken by the T004 production change
- US5 is a **consistency tightening** — it tightens the empty-vault guard to apply in BOTH cases
- Memory hygiene: the production code in T004 does NOT touch the `Zeroizing<String>` binding for the passphrase; the existing `resolve_passphrase_for_env` already returns `Zeroizing<String>`, and the new verify block passes a `&str` view. No new memory concerns.
- 4-layer invariant (Constitution Principle IV): T004 lives in `src/cli/commands.rs::cmd_encrypt` (cli) and calls `crate::core::check_envelope_passphrase` (core, unchanged). No new core helper.
- No-new-error-variants invariant: T004 uses the existing `CliError::PassphraseInput(String)` variant. The new error message is embedded in the `String` payload with a literal `\n` between the sentence and the hint.
- `confirm_key_rotation` removal (T002): the function and its 6-line doc comment are DELETED. The `dialoguer::Confirm` import is removed if unused elsewhere. SC-006 hard acceptance criterion.
