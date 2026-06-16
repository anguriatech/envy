# Tasks: Allow Nested Envy Projects

**Input**: Design documents from `/specs/014-nested-projects/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: The spec explicitly requests tests (2 new unit tests). Write them first per the project's TDD workflow.

**Scale**: ~5 lines of production code across 2 files. Smallest spec in the project's history.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Phase 1: Setup

^- [X] T001 Bump Cargo.toml version from 0.3.1 to 0.3.2 in `/home/oriolgv/aaDev/envy-project/envy/Cargo.toml`

---

## Phase 2: Foundational (Blocking — production-code change)

**Purpose**: The two-file change that enables nested project support. All user stories depend on this.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

^- [X] T002 [P] Change the match arm at lines 57-60 in `cmd_init` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` from `return Err(CliError::ParentProjectExists(...))` to a no-op fall-through (a parent `envy.toml` is now treated as "proceed with init")
^- [X] T003 [P] Delete `ParentProjectExists(String)` variant (lines 35-37) and its exit-code mapping at line 132 in `/home/oriolgv/aaDev/envy-project/envy/src/cli/error.rs`
^- [X] T004 [P] Update `cmd_init`'s doc comment (line 46) in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` to remove the `ParentProjectExists` reference
^- [X] T005 Run `grep -r ParentProjectExists src/` from `/home/oriolgv/aaDev/envy-project/envy/` and verify zero matches (hard acceptance criterion)

**Checkpoint**: `cargo build` succeeds; `grep -r ParentProjectExists src/` returns zero matches.

---

## Phase 3: User Story 1 — Nested init succeeds (Priority: P1)

**Goal**: A user can run `envy init` in a subdirectory of an existing envy project and get a new project with a different UUID.

**Independent Test**: Create a parent project, run `envy init` in a subdirectory, verify exit 0 and different UUID.

### Tests for User Story 1 ⚠️ Write FIRST, ensure they FAIL before implementation

^- [X] T006 [P] [US1] Write failing test `init_nested_succeeds` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (creates parent project in tempdir, creates `child/` subdirectory, cds into it, runs `cmd_init()`, asserts Ok, reads both `envy.toml` files, asserts UUIDs differ)

### Implementation for User Story 1

^- [X] T007 [US1] Verify US1 test passes via `cargo test init_nested_succeeds`

**Checkpoint**: Nested init works.

---

## Phase 4: User Story 2 — AlreadyInitialised still rejected (Priority: P1)

**Goal**: Double-init in the same directory still returns `AlreadyInitialised` — the removal of `ParentProjectExists` must not regress this.

**Independent Test**: Run `envy init` in a directory that already has `envy.toml`, assert `AlreadyInitialised` error.

### Tests for User Story 2 ⚠️ Write FIRST, ensure they FAIL before implementation

^- [X] T008 [P] [US2] Write failing test `init_already_initialised_still_rejected` in `/home/oriolgv/aaDev/envy-project/envy/src/cli/commands.rs` test module (runs `cmd_init()` twice in the same directory, second call asserts `Err(CliError::AlreadyInitialised)`)

### Implementation for User Story 2

^- [X] T009 [US2] Verify US2 test passes via `cargo test init_already_initialised_still_rejected`

**Checkpoint**: Regression test passes — double-init is still blocked.

---

## Phase 5: Polish

^- [X] T010 [P] Add nested projects example to `/home/oriolgv/aaDev/envy-project/envy/README.md` (Quickstart section — "Nested projects (monorepo / multi-project support)" paragraph with directory tree example)
^- [X] T011 [P] Verify `find_manifest_in_parent_dir` test still passes via `cargo test find_manifest_in_parent_dir` in `/home/oriolgv/aaDev/envy-project/envy/`
^- [X] T012 [P] Run `cargo fmt --check` from `/home/oriolgv/aaDev/envy-project/envy/`
^- [X] T013 [P] Run `cargo clippy -- -D warnings` from `/home/oriolgv/aaDev/envy-project/envy/`
^- [X] T014 [P] Run `cargo test` from `/home/oriolgv/aaDev/envy-project/envy/` and verify all tests pass
^- [X] T015 [P] Run `cargo build` from `/home/oriolgv/aaDev/envy-project/envy/`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories.
- **User Stories (Phase 3-4)**: Depend on Foundational.
- **Polish (Phase 5)**: Depends on all user stories.

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational — no dependencies on US2.
- **US2 (P1)**: Can start after Foundational — no dependencies on US1.

### Parallel Opportunities

- T002 + T003 + T004 are `[P]` (different files: `commands.rs` vs `error.rs`)
- T006 (US1 test) and T008 (US2 test) can run in parallel
- T012-T016 (Polish) are all `[P]` (different commands)

---

## Implementation Strategy

### MVP (US1 + US2)

1. Complete Setup (T001)
2. Complete Foundational (T002-T005) — the production code change
3. Complete US1 (T006-T007) — nested init works
4. Complete US2 (T008-T009) — regression test passes
5. Polish (T010-T015) — build, test, lint, docs
6. **DONE** — shippable as 0.3.2

---

## Notes

- Tasks marked `[P]` can run in parallel
- The production change is ~5 lines across 2 files
- `grep -r ParentProjectExists src/` must return zero matches (T005)
- No new crates, no new error variants, no new CLI flags
- `find_manifest` and `create_manifest` are unchanged
