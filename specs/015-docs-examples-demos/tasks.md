---

description: "Task list for Documentation, Examples & Demos feature implementation"
---

# Tasks: Documentation, Examples & Demos

**Input**: Design documents from `/specs/015-docs-examples-demos/`
**Prerequisites**: plan.md (required), spec.md (required — 4 user stories, 11 FRs)

**Tests**: Verification is part of the FRs (E2E Scenario 15 = FR-005, docs check = FR-010) — these are implementation tasks, not optional tests.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: docs/, examples/, vhs/, tests/, .github/ at repository root
- **Zero Rust source changes** (FR-009) — no task may touch src/ or Cargo.toml

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Baseline verification and directory scaffolding

[X] T001 Verify baseline: branch `015-docs-examples-demos` checked out, `cargo build` succeeds, and E2E passes 14 scenarios / 114 assertions (`ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh`) BEFORE any edits
[X] T002 [P] Create directory skeleton: `docs/commands/`, `examples/{basic,team-sync,ci-cd,monorepo}/`, `vhs/` (empty dirs staged via `.gitkeep` if needed)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared templates/contracts that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

[X] T003 Create `docs/commands/_template.md` — the shared command-page template with EXACT headings: `# envy <cmd>`, `## What it does`, `## Aliases`, `## Syntax & flags`, `## Examples`, `## How it works`, `## Related commands` (all 6 `##` sections mandatory; FR-010 grep depends on these exact strings)
[X] T004 [P] Create `examples/README.md` — shared script contract documented: bash `set -euo pipefail`, `ENVY_BIN` support, exports BOTH `ENVY_PASSPHRASE` (keyring fallback → ephemeral vault, no keyring daemon needed) and `ENVY_PASSPHRASE_DEVELOPMENT` (envelope passphrase), dummy values only, idempotent, prints final success line

**Checkpoint**: Template and contract ready — user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Per-command documentation pages (Priority: P1) 🎯 MVP

**Goal**: Every command has a dedicated page under `docs/commands/`, linked from the README command table; CI verifies links and template conformance (FR-001, FR-002, FR-003, FR-010, FR-011)

**Independent Test**: `grep -oE 'docs/commands/envy-[a-z]+\.md' README.md` → all 17 resolve; every `docs/commands/envy-*.md` contains the 6 mandatory `##` headings; run the FR-010 check step locally

### Implementation for User Story 1

[X] T005 [P] [US1] Create `docs/commands/envy-init.md` (template-complete: What it does, Aliases "None", Syntax & flags from `envy init --help`, Examples with dummy values, How it works incl. keyring/master-key, Exit codes, Related commands)
[X] T006 [P] [US1] Create `docs/commands/envy-set.md` (alias none; include `--stdin` and `-e ENV` flags)
[X] T007 [P] [US1] Create `docs/commands/envy-get.md` (pipeline-safe output note)
[X] T008 [P] [US1] Create `docs/commands/envy-list.md` (aliases `ls`)
[X] T009 [P] [US1] Create `docs/commands/envy-rm.md` (aliases `remove`, `unset`)
[X] T010 [P] [US1] Create `docs/commands/envy-run.md` (injection semantics: inherited env + secrets; exit codes 127/N; headless CI note)
[X] T011 [P] [US1] Create `docs/commands/envy-migrate.md` (`.env` import, `-e ENV`)
[X] T012 [P] [US1] Create `docs/commands/envy-encrypt.md` (alias `enc`; strict mode; `ENVY_PASSPHRASE_<ENV>` headless)
[X] T013 [P] [US1] Create `docs/commands/envy-decrypt.md` (alias `dec`; progressive disclosure / multi-key skip)
[X] T014 [P] [US1] Create `docs/commands/envy-export.md` (aliases none; `--format` dotenv/json/shell — `eval $(envy export --format shell)` note)
[X] T015 [P] [US1] Create `docs/commands/envy-diff.md` (alias `df`; exit 0/1 convention; `--reveal` warning)
[X] T016 [P] [US1] Create `docs/commands/envy-status.md` (alias `st`; rotation reminder; `--format json`)
[X] T017 [P] [US1] Create `docs/commands/envy-rotate.md` (passphrase rotation; verifies current first)
[X] T018 [P] [US1] Create `docs/commands/envy-scan.md` (vault-leak scanner; exit 0/1; `--reveal`)
[X] T019 [P] [US1] Create `docs/commands/envy-audit.md` (alias `au`; `--limit N`)
[X] T020 [P] [US1] Create `docs/commands/envy-hooks.md` (pre-commit installer; `--force`; exit 3 conflict)
[X] T021 [P] [US1] Create `docs/commands/envy-completions.md` (bash/zsh/fish/powershell)
[X] T022 [US1] Edit `README.md` — convert the `📋 Full Command Reference` table `Command` column (locate by heading, not line number) into links to `docs/commands/envy-<cmd>.md` for all 17 commands; keep descriptions and alias column (FR-001)
[X] T023 [US1] Edit `README.md` — add a `## Documentation` section after the command reference linking to `docs/commands/`, `examples/`, and `docs/assets/` demo GIFs (FR-011)
[X] T024 [US1] Edit `.github/workflows/ci.yml` — add "Docs check (links + template sections)" step to the `build` job after the E2E steps: (a) every `docs/commands/envy-*.md` link found in README resolves to an existing file, (b) every `docs/commands/envy-*.md` page contains all 6 headings `## What it does`, `## Aliases`, `## Syntax & flags`, `## Examples`, `## How it works`, `## Related commands` (`_template.md` excluded) (FR-010)

**Checkpoint**: US1 complete — README links + all 17 pages + FR-010 check green locally

---

## Phase 4: User Story 2 - CI-verified copy-paste examples (Priority: P1)

**Goal**: Four runnable scenarios under `examples/` verified by E2E Scenario 15 (FR-004, FR-005)

**Independent Test**: `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh` passes Scenario 15 (and all 14 existing scenarios) on Linux/macOS/Windows

### Implementation for User Story 2

[X] T025 [P] [US2] Create `examples/basic/` — `README.md` (init → set → list → run walkthrough, prerequisites), `envy.toml.example` (commented manifest with project_id placeholder), `tutorial.sh` (idempotent: init, set 2 dummy secrets, list, `run -- echo "$DUMMY_KEY"`, prints `basic: OK`)
[X] T026 [P] [US2] Create `examples/team-sync/` — `README.md` (Dev A → Dev B handoff), `dev_a.sh` (init, set dummy secrets, `encrypt -e development` headless via `ENVY_PASSPHRASE_DEVELOPMENT`), `dev_b.sh` (fresh dir, copy `envy.enc` in, `decrypt -e development` headless, verify secret, run, prints `team-sync: OK`)
[X] T027 [P] [US2] Create `examples/ci-cd/` — `README.md` (headless CI usage, exit codes, JSON gates), `workflow.yml` (copyable GitHub Actions workflow: decrypt + status JSON `in_sync` gate), `headless.sh` (init, set, encrypt headless, `status --format json`, diff gates, prints `ci-cd: OK`)
[X] T028 [P] [US2] Create `examples/monorepo/` — `README.md` (nested projects, feature 014), `apps/app-a/envy.toml.example`, `apps/app-b/envy.toml.example`, `script.sh` (init both nested projects fresh in temp dirs — NO committed `envy.toml` to avoid init exit-3 conflict — verify independent secrets, prints `monorepo: OK`)
[X] T029 [US2] Edit `tests/e2e_devops_scenarios.sh` — add `#  15. Documentation examples (envy docs)` to the header scenario list (lines 6-16) and a "Scenario 15 — Documentation examples" section: for each of `examples/{basic,team-sync,ci-cd,monorepo}` create a temp dir, run its scripts via `"$ENVY"` with headless env vars, and assert: basic — `run` prints the env var + `envy.toml` exists; team-sync — `envy.enc` exists + decrypt round-trip succeeds; ci-cd — `status --format json` parses with `jq` and env is `in_sync`; monorepo — both nested projects initialise and resolve secrets independently (FR-005)

**Checkpoint**: US2 complete — all 15 E2E scenarios pass on all 3 OS

---

## Phase 5: User Story 3 - Hero-flow demo videos (Priority: P2)

**Goal**: Three VHS hero tapes, GIFs generated in CI on push to master (FR-006, FR-007)

**Independent Test**: Tapes run locally (`vhs vhs/<name>.tape` produces a GIF); CI `vhs-demos` job passes on master push without manual intervention

### Implementation for User Story 3

[X] T030 [P] [US3] Create `vhs/quickstart.tape` — hero flow (a): init → set 2 dummy secrets → list → `run -- echo`; per VHS best practices: `Output: docs/assets/quickstart.gif` at file start, `Require` envy on PATH, `Env ENVY_PASSPHRASE=<dummy>` + `Env ENVY_PASSPHRASE_DEVELOPMENT=<dummy>`, explicit `Set TypingSpeed`/`Set FontSize`/`Set Width/Height`, `Sleep` after Enter, final `Sleep`, no real secrets
[X] T031 [P] [US3] Create `vhs/team-sync.tape` — hero flow (b): Dev A `encrypt` → show `envy.enc` → Dev B `decrypt` → `run` (same VHS best practices; `Output: docs/assets/team-sync.gif`)
[X] T032 [P] [US3] Create `vhs/ci-cd.tape` — hero flow (c): headless `encrypt` + `status --format json` + diff gates (same VHS best practices; `Output: docs/assets/ci-cd.gif`)
[X] T033 [US3] Edit `.github/workflows/ci.yml` — add `vhs-demos` job: `on: push: branches: [master]` only (FR-007 — fork PRs cannot push), `permissions: contents: write`, steps: checkout with fetch-depth, build release binary, install VHS via `charmbracelet/vhs-action` (verify latest stable version tag at implementation time — v2 expected), run all `vhs/*.tape`, then git add/commit/push `docs/assets/*.gif` ONLY if changed (no-op skip to avoid infinite loops); job fails loudly on tape errors

**Checkpoint**: US3 complete — 3 GIFs generated in CI on master, no drift possible

---

## Phase 6: User Story 4 - Updated developer guide (Priority: P3)

**Goal**: `docs/developer-guide.md` architecture section matches the current source tree (FR-008)

**Independent Test**: Visual scan — every module in `src/` and `src/db/` appears in the guide's structure tree

### Implementation for User Story 4

[X] T034 [US4] Edit `docs/developer-guide.md` section 3 (project-structure tree, lines 71-110) — add missing modules: `src/cli/format.rs`; `src/core/audit.rs`, `src/core/scan.rs`; `src/crypto/strength.rs`, `src/crypto/diceware.rs`; `src/db/audit.rs`, `src/db/sync_markers.rs` (verify actual module list against `ls src/*/` before editing)
[X] T035 [US4] Edit `docs/developer-guide.md` — verify remaining stale references (`rotate_env` in `src/core/sync.rs`, sections 7/8/10-11) and update ONLY what is factually outdated; no rewrite of unrelated content

**Checkpoint**: US4 complete — guide reflects current codebase

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final verification across all user stories

[X] T036 [P] Verify all README → `docs/commands/` links resolve and every page has the 6 mandatory headings (run the FR-010 check locally)
[X] T037 [P] Run full quality gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (must be unaffected — zero Rust changes), `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh` (ALL 15 scenarios pass)
[X] T038 [P] Run `bash examples/basic/tutorial.sh` (with `ENVY_BIN` set) and the other 3 example scripts in temp dirs — all complete cleanly and idempotently (run each twice)
[X] T039 [P] Run `vhs vhs/quickstart.tape` locally (if VHS installed) — produces a GIF without errors
[X] T040 [P] Review all new artifacts for Constitution compliance: English only (Principle V), no real secrets anywhere (Principle I), no `src/` or `Cargo.toml` changes (FR-009)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - US1 (P1) and US2 (P1) can proceed in parallel (different files: docs/ vs examples/+tests)
  - US3 (P2) and US4 (P3) can proceed in parallel after Phase 2
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Depends on T003 (`_template.md`). Pages T005-T021 are fully parallel. T022/T023 (README) depend on the pages. T024 (FR-010 check) depends on T005-T023.
- **User Story 2 (P1)**: Depends on T004 (script contract). Scripts T025-T028 parallel. T029 (Scenario 15) depends on T025-T028.
- **User Story 3 (P2)**: Tapes T030-T032 parallel (all need the release binary built). T033 (CI job) depends on T030-T032.
- **User Story 4 (P3)**: T034 before T035 (structure tree first, then stale references).

### Within Each User Story

- Template/contract before content
- Content before verification (Scenario 15 / FR-010 / CI job)
- Story complete before moving to next priority

### Parallel Opportunities

- All [P]-marked tasks within a phase run in parallel
- US1 pages: up to 17 tasks in parallel (one file each)
- US2 examples: 4 scenarios in parallel (one directory each)
- US3 tapes: 3 tapes in parallel
- US1 + US2 + US3 + US4 can be worked in parallel after Phase 2 (different file trees)

---

## Parallel Example: User Story 1

```bash
# Launch all 17 command pages together (one file each):
Task: "Create docs/commands/envy-init.md"          # T005
Task: "Create docs/commands/envy-set.md"           # T006
...                                              # T007–T021
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (`_template.md` + script contract)
3. Complete Phase 3: User Story 1 (17 pages + README links + FR-010 check)
4. **STOP and VALIDATE**: Run the FR-010 check — all links resolve, all 6 headings present
5. Deploy/demo if ready (docs-only PR, zero risk)

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → README with 17 linked command pages (MVP!)
3. Add User Story 2 → Examples + Scenario 15 → E2E 15 scenarios green
4. Add User Story 3 → VHS tapes + CI demo job
5. Add User Story 4 → developer-guide refresh
6. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 pages (17 files, parallelizable)
   - Developer B: User Story 2 examples
   - Developer C: User Story 3 tapes
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Zero Rust source changes (FR-009) — no task touches `src/` or `Cargo.toml`
- Commit after each task or logical group (e.g. after all pages, after Scenario 15)
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
- All content in English (Constitution V); dummy secret values only (Constitution I)
