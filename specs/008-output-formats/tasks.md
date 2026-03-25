# Tasks: Machine-Readable Output Formats

**Input**: Design documents from `specs/008-output-formats/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, contracts/output-formats.md ✓, quickstart.md ✓

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Exact file paths included in every task description

---

## Phase 1: Setup

**Purpose**: Create the new module file and add it to the module tree.

- [x] T001 Create `src/cli/format.rs` as an empty module (add `pub mod format;` to `src/cli/mod.rs`)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that all three user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T002 Add `OutputFormat` enum with variants `Table`, `Json`, `Dotenv`, `Shell` and derive `clap::ValueEnum`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Default` (default = `Table`) in `src/cli/format.rs` — FR-001, FR-002, FR-003
- [x] T003 Add `OutputData<'a>` enum with variants `SecretList { env, secrets: &[(String,String)] }`, `SecretItem { key, value }`, `ExportList { env, secrets: &[(String,String)] }`, `NotFound { key }` in `src/cli/format.rs` — FR-004, FR-005, FR-006
- [x] T004 Add private `#[derive(serde::Serialize)]` output structs (`SecretPair`, `ListJson`, `ItemJson`, `ExportJson`, `ErrorJson`) in `src/cli/format.rs` — FR-004, FR-005
- [x] T005 Implement private `fmt_table(data: &OutputData, writer: &mut impl Write) -> Result<(), FormatError>` in `src/cli/format.rs`: replicate existing `cmd_list`/`cmd_get` stdout behaviour exactly — FR-011
- [x] T006 Implement private `fmt_json(data: &OutputData, writer: &mut impl Write) -> Result<(), FormatError>` using `serde_json::to_writer` in `src/cli/format.rs` — FR-004, FR-005
- [x] T007 Implement private `fmt_dotenv(data: &OutputData, writer: &mut impl Write) -> Result<(), FormatError>` (`KEY=value\n` per pair) in `src/cli/format.rs` — FR-008
- [x] T008 Implement `fn shell_escape(value: &str) -> String` (`value.replace("'", r"'\''")`) and private `fmt_shell(data: &OutputData, writer: &mut impl Write) -> Result<(), FormatError>` (`export KEY='<escaped>'\n`) in `src/cli/format.rs` — FR-009
- [x] T009 Implement public `pub fn print_output(format: OutputFormat, data: OutputData<'_>, writer: &mut impl Write) -> Result<(), FormatError>` dispatching to the four private helpers in `src/cli/format.rs` — FR-010
- [x] T010 [P] Add `pub fn list_secrets_with_values` to `src/core/ops.rs`: iterates `vault.list_secrets(env_id)`, decrypts each value via `core::crypto::decrypt`, returns `Vec<(String, String)>` in alphabetical order — used by cmd_list and cmd_export
- [x] T011 [P] Add `#[arg(long, short = 'f', global = true, default_value = "table")] pub format: OutputFormat` field to the `Cli` struct in `src/cli/mod.rs` — FR-001, FR-002, FR-003

**Checkpoint**: `cargo build` must succeed before proceeding.

---

## Phase 3: User Story 1 — `envy list --format json` (Priority: P1) 🎯 MVP

**Goal**: CI/CD scripts can parse `envy list --format json` output reliably.

**Independent Test**: `envy init && envy set API_KEY=abc && envy list --format json | jq '.secrets[0].value'` → `"abc"`. Existing `envy list` output must be byte-for-byte unchanged.

- [x] T012 [US1] Write unit tests in `src/cli/format.rs` (inline) covering `fmt_table` (keys only), `fmt_json` (list found/empty), `fmt_dotenv` (basic), `fmt_shell` (basic + single-quote escape), `fmt_json NotFound` shape — SC-001, SC-003
- [x] T013 [US1] Refactor `cmd_list` in `src/cli/commands.rs` to accept `format: OutputFormat`, call `core::ops::list_secrets_with_values`, then `format::print_output(format, OutputData::SecretList { env, secrets: &pairs }, &mut stdout())` — FR-004, FR-011
- [x] T014 [US1] Update `Commands::List` dispatch in `run()` in `src/cli/mod.rs` to pass `cli.format` to `cmd_list` — FR-001, FR-011

**Checkpoint**: `cargo test` passes; `envy list` table output is unchanged; `envy list --format json` emits valid JSON.

---

## Phase 4: User Story 2 — `envy export` command (Priority: P2)

**Goal**: Developers can source secrets into their shell with `eval $(envy export --format shell)` and generate `.env` files with `envy export > .env`.

**Independent Test**: `envy export --format shell` → `export KEY='value'` lines; values with `'` are correctly escaped. `envy export` (no flag) defaults to `dotenv`.

- [x] T015 [US2] Add `Export { #[arg(default_value = "development")] env: String }` variant to `Commands` enum in `src/cli/mod.rs` — FR-006
- [x] T016 [US2] Implement `pub(super) fn cmd_export(vault: &Vault, master_key: &[u8;32], project_id: &ProjectId, env: &str, format: OutputFormat) -> Result<(), CliError>` in `src/cli/commands.rs`: coerce `Table → Dotenv`; call `list_secrets_with_values`; call `print_output(effective, OutputData::ExportList { env, secrets: &pairs }, &mut stdout())` — FR-006, FR-007, FR-008, FR-009
- [x] T017 [US2] Wire `Commands::Export { env }` in `run()` in `src/cli/mod.rs`, passing `cli.format` to `cmd_export` — FR-006, FR-007

**Checkpoint**: `envy export` (no flag) → `KEY=value` lines; `envy export -f shell` → `export KEY='...'` lines.

---

## Phase 5: User Story 3 — `envy get KEY --format json` (Priority: P3)

**Goal**: The future VS Code extension (and scripts) can parse `envy get KEY --format json` without screen-scraping.

**Independent Test**: `envy get EXISTING_KEY --format json` → `{"key":"...","value":"..."}` exit 0; `envy get MISSING --format json` → `{"error":"key not found"}` exit 1.

- [x] T018 [US3] Refactor `cmd_get` in `src/cli/commands.rs` to accept `format: OutputFormat`; on found: `print_output(format, OutputData::SecretItem { key, value: &v }, &mut stdout())`; on not-found: `print_output(format, OutputData::NotFound { key }, &mut stdout())` then return `Err(CliError::Core(e))` — FR-005
- [x] T019 [US3] Update `Commands::Get` dispatch in `run()` in `src/cli/mod.rs` to pass `cli.format` to `cmd_get` — FR-001, FR-005

**Checkpoint**: Both found and not-found JSON shapes match the contract in `contracts/output-formats.md`; exit codes are 0 and 1 respectively.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T020 [P] Unit tests in `src/cli/format.rs` cover `list_json_format`, `get_json_found`, `get_json_not_found` JSON shapes — SC-001, SC-005
- [x] T021 [P] Unit tests in `src/cli/format.rs` cover `export_dotenv_default`, `export_shell_format`, dotenv value with `=` — SC-002
- [x] T022 [P] `--format xml` handled by clap `ValueEnum` validation → exit 2 automatically — FR-012
- [x] T023 `cargo clippy -- -D warnings` — zero warnings
- [x] T024 `cargo fmt` applied to all changed files
- [x] T025 `cargo test` — 71 passed, 0 failed

---

## Dependency Graph

```
T001 → T002 → T003 → T004 → T005 → T006 → T007 → T008 → T009
                                                           ↓
                                                    T010 [P] T011 [P]
                                                           ↓
                             US1: T012 → T013 → T014 (checkpoint)
                                                    ↓
                              US2: T015 → T016 → T017 (checkpoint)
                                                    ↓
                                       US3: T018 → T019 (checkpoint)
                                                    ↓
                               Polish: T020[P] T021[P] T022[P] → T023 → T024 → T025
```

---

## QA Polish (2026-03-25)

Three items addressed after the initial implementation:

- [x] T026 Change `Export.env` from positional to named flag `#[arg(short = 'e', long = "env", value_name = "ENV", default_value = "development")]` in `src/cli/mod.rs` — UX consistency with all other commands
- [x] T027 Update `List` subcommand doc comment in `src/cli/mod.rs` to warn that `--format json|dotenv|shell` decrypts and reveals secret values
- [x] T028 Append Scenario 5 to `tests/e2e_devops_scenarios.sh` covering `envy list --format json`, `envy export -e development --format shell`, and `envy export -e development` (default dotenv) — 11 new assertions, all passing

---

## Implementation Strategy

**MVP** = Phase 1 + Phase 2 + Phase 3 (T001–T014)

Delivers User Story 1 (`envy list --format json`) — the highest-value CI/CD unlock — with zero regression on existing output and full unit test coverage. User Stories 2 and 3 build incrementally on the same formatting engine.
