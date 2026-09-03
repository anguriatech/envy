# Tasks: Interactive TUI (ratatui + crossterm)

**Input**: Design documents from `/specs/016-interactive-tui/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/tui-entry-point.md, quickstart.md

**Tests**: INCLUDED — spec FR-020 explicitly requires unit tests (gradient, filter, mask, lock transitions) and an integration test (bare piped help, no ANSI). Tests must FAIL before implementation (TDD), per Constitution Development Workflow.

**Organization**: Tasks are grouped by user story (US1–US7 from spec.md) so each story is independently implementable and testable.

## Format: `- [ ] [ID] [P?] [Story] Description with file path`

## Path Conventions

- Single Rust binary crate; TUI lives in `src/cli/tui/` (CLI layer, Constitution IV)
- Tests: unit tests inline `#[cfg(test)]` in the module; integration test in `tests/cli_integration.rs`
- Temp-vault test pattern: `tempfile` + `TEST_MASTER_KEY` (existing, commands.rs:2405)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies + module skeleton

- [X] T001 Add `ratatui = "0.29"`, `crossterm = "0.28"`, and `console = "0.15"` to `[dependencies]` in Cargo.toml (research R-001/R-002 — preserves MSRV 1.85)
- [X] T002 [P] Create `src/cli/tui/` module skeleton with doc-header files: `mod.rs`, `app.rs`, `theme.rs`, `banner.rs`, `widgets.rs`, `ops.rs`, `ui.rs` per plan.md structure
- [X] T003 Wire `pub(super) mod tui;` + `pub(super) fn run()` stub in `src/cli/mod.rs`

**Checkpoint**: `cargo build` passes with the new deps; stub compiles

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Terminal lifecycle, core state, data bridge, bare-invocation dispatch — BLOCKS all user stories (no TUI can launch without them)

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Implement terminal lifecycle in `src/cli/tui/mod.rs`: `ratatui::init()` (alternate screen + raw mode + panic restore), `run()` with event loop (`crossterm::event::poll(100ms)` + `read()`), `Ctrl+C` handled as quit (raw mode disables ISIG), `Drop` guard struct that ALWAYS calls restore + `vault.close()` on all exit paths (FR-017/FR-018, research R-004)
- [X] T005 [P] Implement core App state in `src/cli/tui/app.rs`: `VaultState` (Unlocked/Locked per data-model.md), cached projects/environments (non-secret), `BannerState::compact`, `handle_key(key: KeyKind)` stub returning action enum — NO crossterm/ratatui types (pure, testable; research R-008)
- [X] T006 [P] Implement data-loading bridge in `src/cli/tui/ops.rs`: `load_projects()` / `load_environments()` / `load_secrets()` via `core::list_projects`, `core::list_environments`, `core::list_secrets_with_values`; hold `Vault` + `Zeroizing<[u8; 32]>` master key while unlocked (research R-005/R-007)
- [X] T007 Implement dispatch in `src/cli/mod.rs`: `command: Option<Commands>` (clap derive); in `run()` — `None` + `stdout().is_terminal()` → `tui::run()`; `None` + piped → `Cli::command().print_help()` to stderr, return 0, zero ANSI (FR-001/FR-002, contracts/tui-entry-point.md)

**Checkpoint**: `cargo build` + `cargo clippy -- -D warnings` pass; bare `envy` piped prints help to stderr, exit 0 (manual check)

---

## Phase 3: User Story 1 - Launch the interactive TUI (Priority: P1) 🎯 MVP

**Goal**: Bare `envy` in a TTY renders the full-screen TUI with gradient ENVY banner, header (vault state + active env), and quits cleanly with `Q`/`Ctrl+C` restoring the terminal.

**Independent Test**: Run `envy` bare against a TTY with an initialized vault → banner renders, `Q` quits, terminal restored; `B` toggles compact banner.

### Tests for User Story 1 ⚠️ (write first, must FAIL before implementation)

- [X] T008 [P] [US1] Unit tests for gradient in `src/cli/tui/theme.rs` (`#[cfg(test)]`): lerp math between stops `#8A2BE2 → #7B68EE → #9370DB → #1A0933 → #0D0221`, per-line color ordering, 256-quantization, NO_COLOR → plain (FR-004/FR-019/FR-020)

### Implementation for User Story 1

- [X] T009 [P] [US1] Implement palette + gradient in `src/cli/tui/theme.rs`: color stops, `lerp_rgb`, `color_depth()` via `console::Term`, TrueColor → `Color::Rgb` / 256 → quantize / plain fallback; NO_COLOR honored (research R-002)
- [X] T010 [P] [US1] Implement "ENVY" block-letter ASCII banner (const lines, `█`/`╔`/`╚` style) + per-line gradient colors + compact single-line variant in `src/cli/tui/banner.rs` (FR-004/FR-005)
- [X] T011 [US1] Implement header rendering in `src/cli/tui/ui.rs`: banner (full or compact per `BannerState`), `[Locked]`/`[Unlocked]` + active environment indicator; `B` toggle wired to `handle_key` (FR-005/FR-006, depends T009/T010)
- [X] T012 [US1] Render base layout in `src/cli/tui/ui.rs`: header / sidebar area / table area / bottom status bar with hints `[Q] Quit [B] Banner [F] Find [SPACE] Unmask [E] Edit [N] New [D] Delete [S] Sync [L] Lock [U] Unlock` (FR-015); quit on `Q`/`Ctrl+C` flows through the guard (depends T004)

**Checkpoint**: US1 functional — banner + header + quit-to-restore work on a TTY

---

## Phase 4: User Story 2 - Silent execution everywhere else (Priority: P1) 🎯 MVP

**Goal**: Every non-TUI invocation stays byte-identical; bare `envy` piped → help on stderr, exit 0, zero ANSI (regression safety net for the dispatch change).

**Independent Test**: Full existing test-suite + E2E scenarios pass unchanged; new integration test asserts bare piped behavior.

### Tests for User Story 2 ⚠️ (write first, must FAIL before implementation)

- [X] T013 [P] [US2] Integration test in `tests/cli_integration.rs`: run bare `envy` with stdout piped → exit code 0, stderr contains help, stdout empty, no ANSI escape bytes in output (FR-002/FR-020, SC-002)

### Implementation for User Story 2

- [X] T014 [US2] Audit TUI code path for stdout writes: all rendering goes through ratatui alternate screen; no `print!`/`println!`/eprintln in `src/cli/tui/*` (FR-003, contracts: silent guarantees)
- [X] T015 [US2] Regression run: `cargo test` + E2E scenarios (`.github/workflows/ci.yml` E2E script) → verify byte-identical subcommand output vs pre-feature (FR-003, SC-001)

**Checkpoint**: US2 green — dispatch change provably silent; TUI still launches in TTY

---

## Phase 5: User Story 3 - Navigate projects and environments (Priority: P1)

**Goal**: Left sidebar (projects → environments) keyboard-navigable; main panel shows selected env's secrets; header reflects active env; empty states render instead of crashing.

**Independent Test**: Open TUI against a vault with ≥2 projects; navigate with arrows/Tab/Enter; each selection changes the table; empty vault shows hint state.

### Tests for User Story 3 ⚠️ (write first, must FAIL before implementation)

- [X] T016 [P] [US3] Unit tests in `src/cli/tui/app.rs` (`#[cfg(test)]`): sidebar selection state transitions (project switch → env list reload, env switch → active env update), empty-vault and empty-env state construction (FR-020, US3 scenarios 2-3)

### Implementation for User Story 3

- [X] T017 [P] [US3] Implement sidebar rendering in `src/cli/tui/ui.rs`: projects list + nested environments, focus indicator, selected highlight (FR-007)
- [X] T018 [P] [US3] Implement navigation in `src/cli/tui/app.rs`: `↑`/`↓` within list, `←`/`→` or `Enter` to select project/env, `Tab` toggles focus sidebar ↔ table; reload secrets on env switch (FR-007, US3 scenario 1)
- [X] T019 [US3] Sync header active-env indicator with selection + empty states (hint "run `envy init`" / "press `N`") in `src/cli/tui/ui.rs` (US3 scenario 3, edge cases; depends T017/T018)

**Checkpoint**: US3 functional — full navigation loop works on a TTY with a multi-project vault

---

## Phase 6: User Story 4 - Find, reveal, and manage secrets (Priority: P1)

**Goal**: Live search filter, masked-by-default table with per-row `Space` reveal (re-mask on selection change), and `E`/`N`/`D` popups (line editor, masked input, `Ctrl+R` reveal) — full management loop inside the TUI.

**Independent Test**: Search filters live; `Space` reveals then re-masks on move; create → edit → delete round-trip visible via `envy get`/`envy list` outside.

### Tests for User Story 4 ⚠️ (write first, must FAIL before implementation)

- [X] T020 [P] [US4] Unit tests in `src/cli/tui/app.rs`: search filter (case-insensitive substring, live, no-matches), mask/re-mask-on-selection-change state (FR-010/FR-011/FR-020)
- [X] T021 [P] [US4] Unit tests in `src/cli/tui/ops.rs` + `app.rs`: CRUD round-trip against temp vault (`TEST_MASTER_KEY` pattern) — create via `core::set_secret`, edit, delete via `core::delete_secret`; key-validation error (`InvalidSecretKey`) surfaces as status-bar message (FR-012/FR-020)

### Implementation for User Story 4

- [X] T022 [P] [US4] Implement line editor in `src/cli/tui/widgets.rs`: printable chars, backspace, `Esc` cancel, `Enter` confirm, masked-by-default with `Ctrl+R` reveal toggle, `Zeroizing<String>` buffer (FR-012, research R-005)
- [X] T023 [P] [US4] Implement search box in `src/cli/tui/ui.rs` + `app.rs`: `F` focuses, live case-insensitive substring filter on keys, `Esc` closes, "no matches" state (FR-010, US4 scenario 1)
- [X] T024 [P] [US4] Implement secrets table in `src/cli/tui/ui.rs`: key | masked value (`********`) | updated metadata, per-row `Space` reveal, re-mask on selection move, scrolling for long lists (FR-008/FR-009/FR-011)
- [X] T025 [US4] Implement popup state machine in `src/cli/tui/app.rs`: `NewSecret` / `EditSecret` / `DeleteConfirm` variants (data-model.md PopupState); popup open → global hotkeys (`L`/`S`/`Q`/`B`/`F`) ignored (edge case; depends T022)
- [X] T026 [US4] Implement CRUD ops in `src/cli/tui/ops.rs`: create/edit via `core::set_secret`, delete via `core::delete_secret`, error mapping (empty key, `=` in key, empty value) → status-bar messages, table refresh after mutation (FR-012; depends T023/T024/T025)

**Checkpoint**: US4 functional — full manage loop (search, reveal, create, edit, delete) verified against the vault

---

## Phase 7: User Story 5 - Lock, unlock, and sync from the TUI (Priority: P2)

**Goal**: `L` closes the vault + wipes key and values (structure kept); `U` re-fetches keyring key and reloads; `S` runs GitOps sync (env-var passphrases first, else masked popup per env) with working indicator.

**Independent Test**: `L` → header `[Locked]`, table cleared; `U` → reloaded; `S` → `envy.enc` updated, `envy status` reports in sync.

### Tests for User Story 5 ⚠️ (write first, must FAIL before implementation)

- [X] T027 [P] [US5] Unit tests in `src/cli/tui/ops.rs` + `app.rs`: lock/unlock transitions with temp vault (values cleared, structure kept, secrets reload after unlock, keyring failure → stays locked + error surfaced) (FR-013/FR-020)
- [X] T028 [P] [US5] Unit tests for passphrase resolution + sync in `src/cli/tui/ops.rs`: env-var wins over popup path; popup path state entered when no env var; full sync (seal → verify → write artifact) success against temp env + temp artifact path (FR-014/FR-020, research R-006)

### Implementation for User Story 5

- [X] T029 [P] [US5] Implement lock/unlock in `src/cli/tui/ops.rs`: `L` → `vault.close()` + drop `Zeroizing` master key + clear `SecretEntry` values (keep project/env cache); `U` → `crypto::get_or_create_master_key()` + `Vault::open(&cli::vault_path(), key)` + reload (FR-013, research R-007)
- [X] T030 [US5] Implement sync loop in `src/cli/tui/ops.rs`: per env — passphrase from `ENVY_PASSPHRASE_<ENV>` / `ENVY_PASSPHRASE` else masked popup (`widgets.rs`); skip 0-secret envs; `core::check_envelope_passphrase` verify-or-fail (mismatch → error + hint `envy rotate`); `core::seal_artifact` + `core::write_artifact` (FR-014, research R-006; depends T022)
- [X] T031 [US5] Wire `[Locked]`/`[Unlocked]` header state + working indicator during sync + disable table ops while locked in `src/cli/tui/ui.rs` (FR-006/FR-014, depends T029/T030)

**Checkpoint**: US5 functional — lock/unlock round-trip and sync verified via `envy status` outside the TUI

---

## Phase 8: User Story 6 - Security and robustness guarantees (Priority: P2)

**Goal**: Zeroization verified at state level; terminal + vault restoration guaranteed on every exit path; NO_COLOR end-to-end.

**Independent Test**: Unit tests assert zeroized buffers on drop and vault closed on exit; manual smoke: quit, error exit, and panic path restore the terminal.

### Tests for User Story 6 ⚠️ (write first, must FAIL before implementation)

- [X] T032 [P] [US6] Unit tests for zeroization in `src/cli/tui/app.rs` (`#[cfg(test)]`): after lock and after drop, `Zeroizing<String>` value buffers are wiped; no plaintext copy retained in state (FR-016/FR-020, SC-005)
- [X] T033 [P] [US6] Unit tests for guard behavior in `src/cli/tui/mod.rs`: `Drop` guard runs restore + `vault.close()` on normal exit and on error/panic path (simulated via `catch_unwind`), temp vault (FR-017/FR-018/FR-020, SC-006)

### Implementation for User Story 6

- [X] T034 [US6] Zeroize audit across `src/cli/tui/*`: every decrypted value and input buffer in `Zeroizing<String>`; only masked text reaches the draw buffer; no secret in status-bar messages or logs (FR-016, Principle I)
- [X] T035 [US6] NO_COLOR end-to-end pass in `src/cli/tui/theme.rs` + `ui.rs`: with `NO_COLOR` set, banner + status bar + table render without any color codes (FR-019, US2 scenario 3)

**Checkpoint**: US6 green — security guarantees covered by tests; manual panic smoke passed

---

## Phase 9: User Story 7 - Documentation (Priority: P3)

**Goal**: README "Interactive TUI" section + feature plan under `docs/features/` per repo convention.

**Independent Test**: Reviewer follows README section and exercises every documented hotkey successfully.

- [X] T036 [P] [US7] Add "Interactive TUI" section to README.md: launch conditions (bare `envy` + TTY), full hotkey table (contracts/tui-entry-point.md), silent-execution rules, NO_COLOR note (FR-021)
- [X] T037 [P] [US7] Create feature plan under `docs/features/016-interactive-tui/` per repo convention (plan.md + summary), consistent with `specs/016-interactive-tui/` (FR-021)
- [X] T038 [US7] Run quickstart.md validation: `cargo test`, `cargo clippy -- -D warnings`, `cargo audit`, plus the manual TTY smoke checklist (SC-007)

**Checkpoint**: Docs complete; all gates green

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Repo-wide consistency, layer rules, code style

- [X] T039 [P] Layer check (Constitution IV): `src/cli/tui/*` imports only `core`, `crypto::get_or_create_master_key`, `db::Vault::open/close`, `cli` helpers — no other `db`/`crypto` imports
- [X] T040 [P] No-panic audit (Constitution III): no `.unwrap()`/`.expect()` in `src/cli/tui/*` without inline justification comments; typed `Result` via `CliError`
- [X] T041 [P] English-only review (Constitution V) of all new identifiers/comments/messages in `src/cli/tui/*` + README section
- [X] T042 Final full-suite run: `cargo test`, `cargo clippy -- -D warnings`, E2E scenarios, manual TTY smoke per quickstart.md

---

## Phase 11: Review Follow-up - Behavioral Corrections

**Purpose**: Close review findings without changing existing CLI output or core sync APIs.

- [X] T043 [P] [US5] Resolve TUI artifact path from discovered manifest directory; add nested-project regression test (FR-022).
- [X] T044 [P] [US5] Separate envelope sealing from sync-marker commit; update marker only after atomic artifact write; add failed-write regression test (FR-023).
- [X] T045 [P] [US4] Render visible search input/query while filtering; add active-search/state regression coverage (FR-024).
- [X] T046 [P] [US4] Preserve secret `updated_at` metadata through core TUI DTO and render timestamp column; add metadata regression test (FR-025).
- [X] T047 [P] [US6] Render revealed values by borrowing directly from zeroized `App` state without intermediate plaintext allocations; add masking/render regression coverage (FR-025).
- [X] T048 Final review-follow-up validation: `cargo test`, `cargo clippy -- -D warnings`, `cargo audit`, `git diff --check`, nested-cwd and TTY smoke checks.

---

## Phase 12: Project Management and Onboarding UX

- [X] T049 [P] [US3] Add searchable project picker on `P`; compact sidebar to active project and environments (FR-026).
- [X] T050 [P] [US3] Add exact-name project deletion on `X` with environment/secret counts and cascade through core (FR-027/FR-028).
- [X] T051 [P] [US3] Add `?` help popup and contextual hotkey documentation in README/quickstart (FR-029).
- [X] T052 [P] [US3] Add tests for project filtering, deletion cascade, deletion counts, and empty-project state.
- [X] T053 Final UX validation: full suite, clippy, audit, TTY smoke for `P`, `X`, `?`, and documentation review.

---

## Phase 13: Vault Tree and Operations UX

- [X] T054 [P] [US3] Replace lateral project switching with flattened project/environment tree navigation and explicit Enter/Left semantics (FR-030).
- [X] T055 [P] [US5] Load and render per-environment sync indicators, status popup, stale-secret information, and human-readable timestamps (FR-031/FR-032/FR-035).
- [X] T056 [P] [US5] Add active-environment diff popup and artifact-to-vault decrypt/import action with masked passphrase flow (FR-033/FR-034).
- [X] T057 [P] [US3] Add state tests for tree cursor movement, explicit environment selection, status indicators, and operational popup actions.
- [X] T058 Final UX validation: full suite, clippy, audit, TTY smoke for tree navigation, `T`, `G`, and `Y`.

---

## Phase 14: Navigation and Readability Follow-up

- [X] T059 [P] [US3] Render sidebar tree with stateful `ListState` scrolling and preserve highlighted project across context loads (FR-036/FR-037).
- [X] T060 [P] [US3] Render project picker with stateful scrolling for all matches and explicit selection instructions (FR-036/FR-039).
- [X] T061 [P] [US7] Reduce footer to primary controls and rewrite help popup into readable grouped sections (FR-038/FR-039).
- [X] T062 [P] [US3] Add regression tests for long-list cursor visibility, project selection cursor stability, picker search, and help/footer content.
- [X] T063 Final UX validation: full suite, clippy, diff check, TTY smoke with long project list and help/picker flows.

### Phase 18 — UX/Usability review (2026-08-12, FR-040–FR-049)

- [X] T064 [P] [US1] Make top-level `Esc` show a "Press Q to quit" hint instead of exiting; `Q`/`Ctrl+C` remain the only quit keys (FR-040).
- [X] T065 [P] [US6] Add scrollable text popups (Help/Diff/Status) with ↑↓/j/k/PgUp/PgDn/Home/End and a scroll hint in the title (FR-041, `popup_inner_height`/`popup_max_scroll` in `app.rs` + `scroll_popup` in `mod.rs`).
- [X] T066 [P] [US4] Name the affected secret in the Delete confirmation and in the Edit popup title; `Enter` on the New popup key field advances to the value field (FR-042).
- [X] T067 [P] [US6] Add a confirmation popup before `Y` import with an overwrite warning (FR-043, `Popup::ConfirmImport` + `run_import`).
- [X] T068 [P] [US5] Guard vault-reading operations (`N`/`E`/`D`/`S`/`T`/`G`/`Y`/`X`) while locked with a single "Vault locked — press U to unlock" message (FR-044, `require_unlocked`).
- [X] T069 [P] [US4] Distinguish error messages (highlighted) from informational statuses; include the active project in the footer (FR-045, `status_is_error` + `note`/`fail` in `app.rs`, `theme::alert`).
- [X] T070 [P] [US4] Support bracketed paste in popups and search via crossterm `Event::Paste` with control chars stripped (FR-046).
- [X] T071 [P] [US4] Keep the search filter visible in the Secrets panel title and let `Enter` close the search line while keeping the filter (FR-047).
- [X] T072 [P] [US6] Purpose-specific passphrase popup titles (Sync/Diff/Import) and fixed-width passphrase mask (FR-048).
- [X] T073 [P] [US5] Abort the remaining sync queue when the sync passphrase popup is cancelled (FR-049).
- [X] T074 [P] [US6] Validation: full suite (216 tests, 215 passed + 1 pre-existing ignore), clippy `--all-targets -D warnings`, TTY smoke of all new flows (Esc hint, help scroll, delete naming, import confirm, paste, locked guard).

### Phase 19 — TUI workstation redesign (2026-09-03, FR-050–FR-059)

- [X] T075 [US1] Write PRODUCT.md (product truth, Operate mode, brand commitments) and record the direction contract in `ui.rs` header (impeccable shape flow, seed f463bcff).
- [X] T076 [P] [US3] Triptych layout: tree | secrets | Details inspector column, inspector hidden under 100 cols (FR-050).
- [X] T077 [P] [US3] Inspector content: project sync summary, environment state/counts, secret metadata, artifact location, per-selection actions (FR-051).
- [X] T078 [P] [US3] Two-row bottom edge: status row + contextual per-panel key legend (FR-052).
- [X] T079 [P] [US4] Command palette on `:` with searchable action table executed through the same handlers as hotkeys (FR-053).
- [X] T080 [P] [US5] Seal-preview confirmation listing environments + secret counts before any passphrase prompt; empty envs excluded (FR-054).
- [X] T081 [P] [US5] In-TUI rotation flow `R` (current → new → confirm, masked, Ctrl+R reveal) via `core::rotate_env` + `ops::rotate_environment` (FR-055).
- [X] T082 [P] [US5] Seal mismatch recovery: `SealError::Mismatch` surfaces "press R to rotate or Y to import" instead of the `envy rotate` dead-end (FR-056).
- [X] T083 [P] [US4] Panel-scoped `Y`: clipboard copy in secrets (arboard worker thread, clear 30s after last copy), import in tree (FR-057).
- [X] T084 [P] [US6] Functional color semantics: violet focus, green in-sync, amber drift, red errors/destructive, dim metadata; NO_COLOR honored (FR-058).
- [X] T085 [P] [US6] Normalize environment names in `mark_env_sealed` and `rotate_env` artifact lookups (FR-059; fixes "record not found" on mixed-case names).
- [X] T086 [US7] Docs: README TUI section rewrite, hotkey contract table, quickstart checklist (20 steps), data-model (palette/inspector/clipboard), AGENTS.md.
- [X] T087 Validation: full suite (220 tests), clippy `--all-targets -D warnings`, fmt, pty smoke of palette/seal-preview/rotate/copy/legend flows.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — can start immediately
- **Foundational (Phase 2)**: depends on Setup — BLOCKS all user stories (T004 lifecycle, T005 state, T006 bridge, T007 dispatch)
- **US1 (Phase 3)**: depends on T004/T005/T006/T007
- **US2 (Phase 4)**: depends on T007 (dispatch) — runs parallel to US1 after Foundational
- **US3 (Phase 5)**: depends on US1 (layout/table areas) + T006 (data bridge)
- **US4 (Phase 6)**: depends on US3 (table + selection) + T006
- **US5 (Phase 7)**: depends on US4 (popups for passphrase) + T006; independent of US3
- **US6 (Phase 8)**: depends on T004 (guard) + US1 (masked rendering) — parallel to US3/4/5
- **US7 (Phase 9)**: depends on everything (documents shipped behavior)
- **Polish (Phase 10)**: depends on all stories

### User Story Dependencies

- **US1 (P1)**: after Foundational — no story deps → **MVP core**
- **US2 (P1)**: after Foundational — no story deps → **MVP core**
- **US3 (P1)**: after US1 — independently testable once sidebar renders
- **US4 (P1)**: after US3 — independently testable once table + popups exist
- **US5 (P2)**: after US4 (popup reuse) — lock/unlock independently testable
- **US6 (P2)**: after US1 + T004 — independently testable
- **US7 (P3)**: last

### Within Each User Story

- Tests FIRST (TDD, must fail), then models/state, then services (ops.rs), then rendering (ui.rs)
- Story complete before moving to next priority

---

## Parallel Opportunities

- **Setup**: T002 alone (T001 must land first — Cargo.toml)
- **Foundational**: T005 + T006 + T007 parallel after T004 (T004 owns `mod.rs` event loop; others are separate files)
- **US1**: T008 + T009 + T010 parallel; T011/T012 after
- **US2**: T013 (test) + T014 parallel; T015 after T013 green
- **US3**: T016 + T017 + T018 parallel; T019 after
- **US4**: T020 + T021 (tests) + T022 + T023 + T024 parallel; T025 after T022; T026 after T023–T025
- **US5**: T027 + T028 (tests) + T029 parallel; T030 after T022 (line editor for passphrase popup); T031 after T029/T030
- **US6**: T032 + T033 + T034 + T035 parallel
- **US7**: T036 + T037 parallel; T038 after
- **Polish**: T039 + T040 + T041 parallel; T042 last

### Parallel Example: Foundational

```bash
Task: "T005 App core state in src/cli/tui/app.rs"
Task: "T006 Data bridge in src/cli/tui/ops.rs"
Task: "T007 Dispatch in src/cli/mod.rs"
```

### Parallel Example: User Story 4

```bash
Task: "T020 filter + mask unit tests in app.rs"
Task: "T022 line editor in widgets.rs"
Task: "T023 search box in ui.rs + app.rs"
Task: "T024 secrets table in ui.rs"
```

---

## Implementation Strategy

### MVP First (US1 + US2)

1. Complete Phase 1: Setup (T001–T003)
2. Complete Phase 2: Foundational (T004–T007 — CRITICAL, blocks all)
3. Complete US1: banner + header + quit (T008–T012)
4. Complete US2: silence guarantees (T013–T015)
5. **STOP and VALIDATE**: TUI launches on TTY with banner; bare piped is silent; full existing suite green
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. US1 + US2 → launch + silence → **MVP**
3. US3 → navigation → test independently
4. US4 → manage loop (search/reveal/CRUD) → test independently
5. US5 → lock/unlock + sync → test independently
6. US6 → security tests/audit → test independently
7. US7 → docs → demo

### Parallel Team Strategy

1. Team completes Setup + Foundational together
2. Then: Developer A → US1+US3+US4 (UI chain), Developer B → US2 (silence tests) then US6 (security), Developer C → US5 (sync/lock, after T022 popup)
3. Stories integrate independently; no shared-file conflicts (ui.rs chain vs ops.rs vs mod.rs)

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to spec.md user story
- Every phase is independently completable and testable (Independent Test per phase)
- Tests written first and confirmed failing before implementation (TDD)
- Commit after each task or logical group
- Verify `cargo test` + `cargo clippy -- -D warnings` after every phase
