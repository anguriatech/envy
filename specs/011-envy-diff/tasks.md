# Tasks: Pre-Encrypt Secret Diff

**Input**: Design documents from `/specs/011-envy-diff/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/diff-command.md

**Tests**: TDD enforced in Phase 1 — unit tests written before implementation. CLI tests in Phase 2. E2E tests in Phase 3.

**Organization**: 3 phases per user request. Phase 1 covers core logic (US1 foundation). Phase 2 covers CLI orchestration and rendering (US1–US5). Phase 3 covers E2E validation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Core Logic & Unit Tests (TDD)

**Purpose**: Define the core diff types and pure `compute_diff()` function using strict TDD — tests first, implementation second. No CLI, no I/O, no crypto changes.

### Tests (write first — must FAIL before implementation)

- [x] T001 [P] Create `src/core/diff.rs` with `ChangeType` enum, `DiffEntry` struct, `DiffReport` struct (types only, no function body). Add `pub mod diff;` to `src/core/mod.rs` and re-export `ChangeType`, `DiffEntry`, `DiffReport`, `compute_diff`. Provide a `compute_diff` stub that returns an empty report so the module compiles but tests will fail on assertions.
- [x] T002 [P] Add unit test `diff_all_added` in `src/core/diff.rs`: vault has keys {A, B}, artifact is empty. Assert 2 `Added` entries, sorted alphabetically, `has_differences() == true`.
- [x] T003 [P] Add unit test `diff_all_removed` in `src/core/diff.rs`: vault is empty, artifact has keys {X, Y}. Assert 2 `Removed` entries, sorted alphabetically.
- [x] T004 [P] Add unit test `diff_all_modified` in `src/core/diff.rs`: vault has {A="new"}, artifact has {A="old"}. Assert 1 `Modified` entry with `old_value == "old"` and `new_value == "new"`.
- [x] T005 [P] Add unit test `diff_mixed_changes` in `src/core/diff.rs`: vault {A="same", B="new_val", D="added"}, artifact {A="same", B="old_val", C="removed"}. Assert 3 entries: B=Modified, C=Removed, D=Added. Key A excluded (identical values). Counts: added=1, removed=1, modified=1.
- [x] T006 [P] Add unit test `diff_no_changes` in `src/core/diff.rs`: vault {A="v"}, artifact {A="v"}. Assert empty entries, `has_differences() == false`, `total() == 0`.
- [x] T007 [P] Add unit test `diff_empty_both` in `src/core/diff.rs`: both empty BTreeMaps. Assert empty entries, `has_differences() == false`.
- [x] T008 [P] Add unit test `diff_sorted_output` in `src/core/diff.rs`: vault has keys {Z, A, M}, artifact is empty. Assert entries are ordered [A, M, Z].
- [x] T009 [P] Add unit test `diff_values_retained_for_modified` in `src/core/diff.rs`: vault {K="new_secret"}, artifact {K="old_secret"}. Assert `old_value == Some("old_secret")` and `new_value == Some("new_secret")` on the Modified entry.

### Implementation (make tests pass)

- [x] T010 Implement `compute_diff()` in `src/core/diff.rs`: iterate the union of keys from both sorted BTreeMaps. For each key — present in vault only → Added, present in artifact only → Removed, present in both with differing values → Modified, identical → skip. Build `DiffReport` with sorted entries and counts. All values wrapped in `Zeroizing<String>`.

**Checkpoint**: Run `cargo test` — all 8 core diff tests (T002–T009) must pass. Run `cargo clippy -- -D warnings` to verify no warnings.

---

## Phase 2: CLI Orchestration & Formatting (US1–US5)

**Purpose**: Wire the `Diff` command variant, implement `cmd_diff` orchestration, table and JSON renderers, error variants, and colored output. Covers all 5 user stories.

### Error Variants (foundation for all CLI work)

- [x] T011 [P] Add `EnvNotFound(String)` variant to `CliError` in `src/cli/error.rs`. Map to exit code 3 in `cli_exit_code()`. Add unit test `env_not_found_maps_to_exit_code_3`.
- [x] T012 [P] Add `ArtifactUnreadable(String)` variant to `CliError` in `src/cli/error.rs`. Map to exit code 5 in `cli_exit_code()`. Add unit test `artifact_unreadable_maps_to_exit_code_5`.

### Command Variant & Dispatch

- [x] T013 Add `Diff` variant to `Commands` enum in `src/cli/mod.rs` with `-e`/`--env` (default "development"), `--reveal` (bool), and `#[command(visible_alias = "df")]`. Add dispatch arm in `run()` that calls `commands::cmd_diff()` and maps `Ok(true) → 1`, `Ok(false) → 0`, `Err(e) → cli_exit_code(&e)`. (spec.md US1 Acceptance Scenario 4, contracts/diff-command.md §Exit Codes)

### Color Helpers

- [x] T014 [P] [US1] Implement private `is_color_enabled()` and `colorize()` helpers in `src/cli/commands.rs`. `is_color_enabled()` returns `false` if `NO_COLOR` env var is set or stdout is not a TTY (use `std::io::IsTerminal`). `colorize(text, ansi_code)` wraps text in ANSI escape sequences when color is enabled. (plan.md §Color Helper, research.md R3)

### Table Renderer (US1 + US2)

- [x] T015 [US1] Implement private `render_diff_table(report: &DiffReport, reveal: bool, artifact_missing: bool, env_not_in_artifact: bool)` in `src/cli/commands.rs`. Output format per contracts/diff-command.md §Standard Output — Table Format: header line `envy diff: {env} (vault ↔ envy.enc)`, optional notice for missing artifact or missing env in artifact, each entry as `  + KEY` (green/32), `  - KEY` (red/31), `  ~ KEY` (yellow/33), summary line `N changes: X added, Y removed, Z modified`, and `envy diff: {env} — no differences` when empty. (spec.md US1 Acceptance Scenarios 1–2, contracts/diff-command.md §Differences found, §No differences, §Artifact not found)
- [x] T016 [US2] Extend `render_diff_table` to handle `reveal == true`: for Added entries print indented `vault:    {value}`, for Removed entries print `artifact: {value}`, for Modified entries print both `artifact: {old}` and `vault:    {new}`. Value lines indented 4 spaces for visual grouping. (spec.md US2 Acceptance Scenarios 1–3, contracts/diff-command.md §Differences found with --reveal)

### JSON Writer (US3)

- [x] T017 [US3] Implement private `write_diff_json(report: &DiffReport, reveal: bool, writer: &mut impl Write) -> Result<(), CliError>` in `src/cli/commands.rs`. Build JSON via `serde_json::json!()` per contracts/diff-command.md §JSON field contracts: root object with `environment`, `has_differences`, `summary` (added/removed/modified/total), and `changes` array (each with `key` and `type`). When `reveal == true`, conditionally insert `old_value`/`new_value` keys (null for absent sides). When `reveal == false`, these keys must be entirely absent. (spec.md US3 Acceptance Scenarios 1–4, research.md R5)
- [x] T018 [P] [US3] Add unit test `diff_json_no_reveal` in `src/cli/commands.rs`: create a DiffReport with 1 Added + 1 Modified entry, call `write_diff_json` to `Vec<u8>` with `reveal=false`, parse JSON, assert no `old_value`/`new_value` keys exist in any change entry. Assert `has_differences: true`.
- [x] T019 [P] [US3] Add unit test `diff_json_with_reveal` in `src/cli/commands.rs`: same DiffReport, `reveal=true`. Assert `old_value`/`new_value` keys are present in every change entry. Assert null values where appropriate (old_value null for Added, new_value null for Removed).
- [x] T020 [P] [US3] Add unit test `diff_json_no_differences` in `src/cli/commands.rs`: empty DiffReport, `reveal=false`. Assert `has_differences: false`, empty `changes` array, all summary counts 0.
- [x] T021 [P] [US3] Add unit test `diff_json_type_strings` in `src/cli/commands.rs`: DiffReport with all three change types. Assert type values are exactly `"added"`, `"removed"`, `"modified"` (lowercase strings, not enum names).

### Command Handler Orchestration (US1 + US4 + US5)

- [x] T022 [US1] Implement `cmd_diff()` in `src/cli/commands.rs` with full orchestration flow per plan.md §cmd_diff Handler: (1) fetch vault secrets via `core::get_env_secrets`, convert HashMap → BTreeMap; (2) read artifact via `core::read_artifact`, handle `SyncError::FileNotFound` as artifact_missing=true with empty BTreeMap, handle other errors as `ArtifactUnreadable`; (3) if artifact exists and `artifact.environments.contains_key(env_name)`, resolve passphrase via `resolve_passphrase_for_env(env_name, false, None)` and unseal via `core::unseal_env` — if `Ok(None)` return `PassphraseInput("incorrect passphrase for environment '...'")`; (4) if both sides empty, return `EnvNotFound`; (5) call `core::compute_diff`; (6) if `reveal`, print warning to stderr; (7) render based on format (Json → `write_diff_json`, else → `render_diff_table`); (8) return `Ok(report.has_differences())`. (spec.md US1 AS1–4, US4 AS1–3, US5 AS1–3, contracts/diff-command.md §Passphrase Resolution)

**Checkpoint**: Run `cargo test` — all Phase 1 tests + Phase 2 JSON tests (T018–T021) must pass. Run `cargo clippy -- -D warnings`. Run `cargo check` to verify the full build compiles.

---

## Phase 3: E2E Testing & Polish

**Purpose**: End-to-end validation via the bash E2E script. Verify the full round-trip with the compiled binary. Final quality gate.

### E2E Scenario

- [x] T023 Add Scenario 9 (envy diff round-trip) to `tests/e2e_devops_scenarios.sh`: (1) Init project in temp dir; (2) Set 3 secrets (A=a, B=b, C=c); (3) Encrypt with `ENVY_PASSPHRASE`; (4) Add D=d, modify B=b2, remove C via `envy rm`; (5) Run `envy diff` with `ENVY_PASSPHRASE` — assert exit code 1; (6) Run `envy diff --format json` — pipe to `jq`, assert `.summary.added == 1`, `.summary.removed == 1`, `.summary.modified == 1`, `.summary.total == 3`; (7) Assert no `old_value`/`new_value` keys in changes (no `--reveal`); (8) Re-encrypt to bring vault in sync; (9) Run `envy diff` — assert exit code 0 (no differences); (10) Run `envy diff --format json` — assert `.has_differences == false` and `.changes | length == 0`.

### Polish

- [x] T024 Run `cargo fmt` to format all modified files.
- [x] T025 Run `cargo clippy -- -D warnings` and fix any new warnings in `src/core/diff.rs`, `src/cli/mod.rs`, `src/cli/commands.rs`, `src/cli/error.rs`.
- [x] T026 Run `cargo audit` and verify no new advisories from unchanged `Cargo.toml`.
- [x] T027 Update `CLAUDE.md` — add 011-envy-diff to Recent Changes section, add `src/core/diff.rs` to Project Structure, update Commands section to include `Diff` variant.

**Checkpoint**: Run full `cargo test`. Run E2E script directly: `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh`. All scenarios pass (including new Scenario 9).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Core Logic)**: No dependencies — can start immediately. Pure Rust with no external calls.
- **Phase 2 (CLI)**: Depends on Phase 1 completion (T010 — `compute_diff` must exist and pass tests). T011/T012 (error variants) can start in parallel with Phase 1 since they touch `src/cli/error.rs` (different file).
- **Phase 3 (E2E + Polish)**: Depends on Phase 2 completion (T022 — `cmd_diff` must be wired and functional).

### Within Phase 1 (TDD)

```
T001 (types + stub) ──► T002–T009 (tests, all [P]) ──► T010 (implement compute_diff)
```

- T001 must come first (creates the file and stub so tests compile).
- T002–T009 can all be written in parallel (they all test the same function).
- T010 depends on all tests existing (TDD: tests first).

### Within Phase 2

```
T011, T012 (error variants, [P]) ──► T013 (command variant + dispatch)
T014 (color helpers, [P])         ──► T015 (table renderer) ──► T016 (reveal in table)
                                      T017 (JSON writer) ──► T018–T021 (JSON tests, [P])
                                      T022 (cmd_diff orchestration, depends on T013+T015+T017)
```

- T011/T012 are independent of each other ([P]) and can start in parallel with Phase 1.
- T014 is independent ([P]) — touches different code section.
- T015 and T017 can run in parallel (table renderer vs JSON writer — same file but different functions).
- T022 depends on T013 (dispatch), T015 (table), T17 (JSON) — assembles everything.

### Parallel Opportunities

```text
# Phase 1 — all tests at once after stub:
T002, T003, T004, T005, T006, T007, T008, T009  (8 parallel tasks)

# Phase 2 — error variants + color helper simultaneously:
T011, T012, T014  (3 parallel tasks)

# Phase 2 — renderers simultaneously:
T015, T017  (2 parallel tasks, same file but different functions)

# Phase 2 — JSON tests simultaneously:
T018, T019, T020, T021  (4 parallel tasks)
```

---

## Implementation Strategy

### MVP (Phase 1 + Phase 2 through T022)

1. Complete Phase 1: Core logic with full test coverage (T001–T010)
2. Complete Phase 2: CLI wiring with error variants, renderers, and orchestration (T011–T022)
3. **STOP and VALIDATE**: `cargo test` + `cargo clippy` + manual smoke test
4. User can now run `envy diff`, `envy diff --reveal`, `envy diff --format json`

### Full Delivery (Phase 3)

5. Add E2E Scenario 9 for regression protection (T023)
6. Polish: fmt, clippy, audit, CLAUDE.md (T024–T027)
7. **FINAL GATE**: Full test suite + E2E script passes on all scenarios

---

## Notes

- Zero new crate dependencies — all tasks use existing libraries (research.md R7)
- Zero schema migrations — all data structures are transient (data-model.md)
- `cmd_diff` is the only handler returning `Result<bool, CliError>` — this is intentional (research.md R4)
- The `contains_key()` check before `unseal_env()` is critical for disambiguating "env not in artifact" from "wrong passphrase" (research.md R1)
- All `Zeroizing<String>` values in `DiffEntry` are zeroed on drop — no special cleanup needed
