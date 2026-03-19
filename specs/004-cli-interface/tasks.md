# Tasks: CLI Interface

**Feature**: 004-cli-interface
**Input**: spec.md, plan.md, contracts/cli.md
**Total tasks**: 40

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[US?]**: Which user story this task belongs to
- Every test from `plan.md` has its own explicit task
- Tests are written **before** their corresponding implementation (TDD)

---

## Phase 1: Setup

**Purpose**: Add the `dirs` dependency, wire `main.rs` to the CLI entry point, and scaffold the `src/cli/` file structure with complete clap argument definitions. All subsequent phases write into these scaffolded files.

- [x] T001 Add `dirs = "5"` to `[dependencies]` in `Cargo.toml`
- [x] T002 Replace the stub body of `fn main()` in `src/main.rs` with: `use std::process; fn main() { process::exit(envy::cli::run()); }`
- [x] T003 Create `src/cli/commands.rs` with a module-level doc comment (`//! Command handler functions — see plan.md §4.2`) and a single `// TODO: implement` placeholder; create `src/cli/error.rs` with a module-level doc comment (`//! CLI-specific errors and exit-code mapping — see contracts/cli.md`) and a `// TODO: implement` placeholder
- [x] T004 Rewrite `src/cli/mod.rs`: add `mod commands; mod error;`; derive `clap::Parser` on `Cli` struct (`#[command(name = "envy", version, about)]`) with a single `#[command(subcommand)] pub command: Commands` field; derive `clap::Subcommand` on `Commands` enum with exactly these 7 variants — `Init`, `Set { assignment: String, #[arg(short='e', long="env")] env: Option<String> }`, `Get { key: String, env: Option<String> }`, `List { env: Option<String> }` with `#[command(alias = "ls")]`, `Rm { key: String, env: Option<String> }` with `#[command(alias = "remove")]`, `Run { env: Option<String>, #[arg(last = true, required = true)] command: Vec<String> }`, `Migrate { file: std::path::PathBuf, env: Option<String> }`; add `pub(super) fn vault_path() -> std::path::PathBuf` that returns `dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".envy").join("vault.db")`; add `pub fn run() -> i32 { todo!("dispatch not yet wired") }` stub; run `cargo build` and confirm it compiles

**Checkpoint**: `cargo build` succeeds. `envy --help` lists all 7 subcommands with correct flags.

---

## Phase 2: `CliError` Enum and Formatting (Foundational)

**Purpose**: Define the typed CLI error surface and exit-code table. All Phase 3–5 error-handling tasks depend on this phase being complete.

**⚠️ CRITICAL**: No command handler implementation can begin until T005–T007 are complete.

- [x] T005 Implement the `CliError` enum in `src/cli/error.rs` using `#[derive(Debug, thiserror::Error)]` with exactly these 6 variants and display strings from `contracts/cli.md`: `InvalidAssignment(String)` → `"invalid assignment \"{0}\": expected KEY=VALUE format"`, `FileNotFound(String, String)` → `"cannot read file \"{0}\": {1}"`, `AlreadyInitialised` → `"already initialised: envy.toml exists in this directory"`, `ParentProjectExists(String)` → `"parent project detected: \"{0}\" already contains envy.toml"`, `ProjectNotInVault` → `"project not found in vault — was the vault file moved?"`, `VaultOpen(String)` → `"could not open vault: {0}"`
- [x] T006 Add `pub fn format_core_error(e: &crate::core::CoreError) -> String` and `pub fn format_cli_error(e: &CliError) -> String` to `src/cli/error.rs`; both functions return `format!("error: {}", e)` (the `Display` impl on the error type provides the human-readable message); add the necessary `pub use error::{CliError, format_core_error, format_cli_error};` re-exports in `src/cli/mod.rs`
- [x] T007 Add `pub fn core_exit_code(e: &crate::core::CoreError) -> i32` and `pub fn cli_exit_code(e: &CliError) -> i32` to `src/cli/error.rs` with exit-code logic matching the table in `contracts/cli.md`: `ManifestNotFound` → 1, `Db(DbError::NotFound)` → 1, `ManifestInvalid` → 1, `ManifestIo` → 1, `InvalidSecretKey` → 2, `Db(_)` → 4, `Crypto(_)` → 4; `CliError::InvalidAssignment` → 2, `FileNotFound` → 1, `AlreadyInitialised` → 3, `ParentProjectExists` → 3, `ProjectNotInVault` → 4, `VaultOpen` → 4; add `pub use error::{core_exit_code, cli_exit_code};` to `src/cli/mod.rs`; run `cargo build`

**Checkpoint**: `cargo build` succeeds; `envy::cli::CliError`, `format_core_error`, `format_cli_error`, `core_exit_code`, `cli_exit_code` are all accessible.

---

## Phase 3: Core Command Handlers — TDD (US1: `init` + US2: `set`/`get`/`list`/`rm`)

**Goal**: Implement and test the 5 fundamental CRUD commands. After this phase the tool can initialise a project and fully manage secrets, independently of `run` or `migrate`.

**Independent Test**: `cargo test` passes all unit tests; a project can be initialised and secrets round-trip correctly through set/get/list/rm.

### Tests — write first, verify they FAIL/COMPILE before implementation (T008–T015)

- [x] T008 [US2] Write test `parse_assignment_basic` in `src/cli/commands.rs` (`#[cfg(test)]`): assert `"KEY=VALUE".split_once('=')` returns `Some(("KEY", "VALUE"))`
- [x] T009 [US2] Write test `parse_assignment_value_contains_equals` in `src/cli/commands.rs`: assert `"TOKEN=abc=def".split_once('=')` returns `Some(("TOKEN", "abc=def"))` — confirms first-`=`-only split rule
- [x] T010 [US2] Write test `parse_assignment_no_equals` in `src/cli/commands.rs`: assert `"NOVALUE".split_once('=')` returns `None` — the case that must produce `CliError::InvalidAssignment`
- [x] T011 [US2] Write test `parse_assignment_empty_key` in `src/cli/commands.rs`: assert `"=VALUE".split_once('=')` returns `Some(("", "VALUE"))` — confirms the key is empty string, which Core will reject via `InvalidSecretKey`
- [x] T012 [US1] Write test `format_manifest_not_found` in `src/cli/error.rs`: call `format_core_error(&crate::core::CoreError::ManifestNotFound)`; assert the returned string starts with `"error: "` and contains the substring `"envy init"` (from the CoreError display message)
- [x] T013 [US1] Write test `exit_code_not_found` in `src/cli/error.rs`: assert `core_exit_code(&crate::core::CoreError::ManifestNotFound) == 1`; also assert `core_exit_code(&crate::core::CoreError::Db(crate::db::DbError::NotFound)) == 1`
- [x] T014 [US2] Write test `exit_code_invalid_key` in `src/cli/error.rs`: assert `core_exit_code(&crate::core::CoreError::InvalidSecretKey(String::new())) == 2`
- [x] T015 Run `cargo test --no-run` to confirm all tests in `src/cli/` compile; the parsing tests (T008–T011) should pass immediately; error-formatting tests (T012–T014) may need `use` imports but must compile

### Implementation (T016–T021)

- [x] T016 [US1] Implement `pub(super) fn cmd_init() -> Result<(), CliError>` in `src/cli/commands.rs`: (1) `std::env::current_dir()` → `cwd`; (2) call `crate::core::find_manifest(&cwd)` — if `Ok(_)` return `Err(CliError::AlreadyInitialised)`; if `Err(CoreError::ManifestNotFound)` continue; if `Err(other)` return `Err(CliError::VaultOpen(other.to_string()))`; (3) walk ancestors: if any dir above `cwd` satisfies `find_manifest` returning `Ok((_, found_dir))` where `found_dir != cwd`, return `Err(CliError::ParentProjectExists(found_dir.display().to_string()))`; (4) `let project_id = uuid::Uuid::new_v4().to_string()`; (5) `let master_key = crate::crypto::get_or_create_master_key(&project_id).map_err(|e| CliError::VaultOpen(e.to_string()))?`; (6) `let vault = crate::db::Vault::open(&super::vault_path(), master_key.as_ref()).map_err(|e| CliError::VaultOpen(e.to_string()))?`; (7) `vault.create_project(&crate::db::ProjectId(project_id.clone())).map_err(|e| CliError::VaultOpen(e.to_string()))?`; (8) `crate::core::create_manifest(&cwd, &project_id).map_err(|e| CliError::VaultOpen(e.to_string()))?`; (9) `println!("✓ Initialised envy project {}.", project_id)`; return `Ok(())`
- [x] T017 [US2] Implement `pub(super) fn cmd_set(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, env: &str, assignment: &str) -> Result<(), crate::core::CoreError>` in `src/cli/commands.rs`: split `assignment` via `assignment.split_once('=').ok_or_else(|| { /* map to CoreError::InvalidSecretKey? No — CliError */ })`; because the return type is `CoreError` but missing `=` is a `CliError`, change the return type to `Result<(), Box<dyn std::error::Error>>` OR handle the split in the caller (`run()`) and pass `key` and `value` separately — **preferred**: change signature to accept `key: &str, value: &str` (splitting is done in `run()` before dispatch); call `crate::core::set_secret(vault, master_key, project_id, env, key, value)?`; print `"✓ Set {} in {}.", key, effective_env`; return `Ok(())`

  > **Note on signature**: The split of `assignment` into `(key, value)` MUST happen in `run()` before dispatching to `cmd_set`. This keeps `cmd_set` focused on Core delegation. Update the handler signature accordingly if needed and keep it consistent with the contracts.

- [x] T018 [US2] Implement `pub(super) fn cmd_get(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, env: &str, key: &str) -> Result<(), crate::core::CoreError>` in `src/cli/commands.rs`: call `crate::core::get_secret(vault, master_key, project_id, env, key)?`; print exactly `"{}\n", *value` via `println!` (no labels, no extra text) — this is the UNIX-pipeline-safe output; return `Ok(())`
- [x] T019 [US2] Implement `pub(super) fn cmd_list(vault: &crate::db::Vault, project_id: &crate::db::ProjectId, env: &str) -> Result<(), crate::core::CoreError>` in `src/cli/commands.rs`: call `crate::core::list_secret_keys(vault, project_id, env)?`; print each key on its own line via `println!("{k}")`; if the list is empty print `eprintln!("(no secrets in {})", env_or_default)` to stderr; return `Ok(())`
- [x] T020 [US2] Implement `pub(super) fn cmd_rm(vault: &crate::db::Vault, project_id: &crate::db::ProjectId, env: &str, key: &str) -> Result<(), crate::core::CoreError>` in `src/cli/commands.rs`: call `crate::core::delete_secret(vault, project_id, env, key)?`; print `"✓ Deleted {} from {}.", key, effective_env`; return `Ok(())`
- [x] T021 Run `cargo test` — all unit tests in `src/cli/commands.rs` and `src/cli/error.rs` must pass with zero failures

**Checkpoint**: `cargo test` — all parsing tests, error formatting tests, and exit-code tests pass.

---

## Phase 4: Complex Command Handlers — TDD (US3: `run` + US4: `migrate`)

**Goal**: Implement process injection and legacy migration. After this phase all 7 command handlers are complete, but not yet wired to a working `run()` dispatcher.

**Independent Test**: `cargo test` passes all unit tests including migrate parsing tests.

### Tests — write first, verify they FAIL/COMPILE before implementation (T022–T024)

- [ ] T022 [US4] Write test `migrate_skips_comments_and_blanks` in `src/cli/commands.rs` (`#[cfg(test)]`): define a multiline `&str` with 2 valid `KEY=VALUE` lines, 1 comment line (`# comment`), and 1 blank line; parse line-by-line using `split_once('=')` after trimming and comment/blank checks; assert exactly 2 `(key, value)` tuples are produced and 0 lines are flagged as malformed
- [ ] T023 [US4] Write test `migrate_warns_on_malformed` in `src/cli/commands.rs`: define a multiline `&str` with 1 valid `KEY=VALUE` line and 1 malformed line (`"BADLINE"` with no `=`); parse line-by-line; assert 1 valid pair is produced and `split_once` returns `None` for exactly 1 line (the malformed one that would trigger a warning)
- [ ] T024 Run `cargo test --no-run` to confirm T022–T023 compile

### Implementation (T025–T027)

- [ ] T025 [US3] Implement `pub(super) fn cmd_run(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, env: &str, command: &[String]) -> i32` in `src/cli/commands.rs`: call `crate::core::get_env_secrets(vault, master_key, project_id, env)` — on error print `eprintln!("{}", format_core_error(&e))` and return `core_exit_code(&e)`; split `command` into `(bin, args) = command.split_first().expect("clap ensures non-empty")`; call `std::process::Command::new(bin).args(args).envs(secrets.iter().map(|(k, v)| (k.as_str(), v.as_str()))).status()`; on `Ok(status)` return `status.code().unwrap_or(1)`; on `Err(e)` print `eprintln!("error: failed to execute `{}`: {}", bin, e)` and return `127`
- [ ] T026 [US4] Implement `pub(super) fn cmd_migrate(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, env: &str, file: &std::path::Path) -> Result<(), CliError>` in `src/cli/commands.rs`: read file with `std::fs::read_to_string(file).map_err(|e| CliError::FileNotFound(file.display().to_string(), e.to_string()))?`; iterate `.lines().enumerate()`; skip lines where `trimmed.is_empty() || trimmed.starts_with('#')`; for each remaining line call `trimmed.split_once('=')`: on `Some((key, value))` call `crate::core::set_secret(vault, master_key, project_id, env, key.trim(), value)?` (propagate `CoreError` as `?`, mapping via `From`... or return `Result<(), Box<dyn Error>>`); on `None` print `eprintln!("warning: line {}: skipping malformed entry: {:?}", line_no + 1, trimmed)` and increment a warning counter; after all lines print `"✓ Imported {} secret(s) into {}{}.", imported, env, warnings_suffix`; return `Ok(())`

  > **Note on error type**: `cmd_migrate` returns `Result<(), CliError>` per the contract. `CoreError` from `set_secret` cannot be directly `?`-propagated into `CliError` unless a `From` impl exists. Solution: wrap it — `core::set_secret(...).map_err(|e| CliError::VaultOpen(e.to_string()))?` or change the return type. Prefer wrapping for MVP simplicity.

- [ ] T027 Run `cargo test` — all unit tests in `src/cli/commands.rs` must pass; specifically verify T022 and T023 pass

**Checkpoint**: `cargo test` — all 9 unit tests pass. All 7 handler functions exist in `src/cli/commands.rs` (even if `run()` in `mod.rs` still panics).

---

## Phase 5: Dispatch Logic + Integration Tests (US1–US4)

**Goal**: Wire `pub fn run() -> i32` in `mod.rs` to fully dispatch all 7 commands with complete vault lifecycle management. Write integration tests first (TDD), then implement dispatch.

**Independent Test**: `cargo test` (all tests, including integration) passes.

### Integration tests — write first, verify they compile before dispatch implementation (T028–T035)

- [x] T028 [US1] Write integration test `cli_init_creates_manifest` in `tests/cli_integration.rs`: use a `tempfile::tempdir()` as working dir; spawn `std::process::Command::new(env!("CARGO_BIN_EXE_envy")).arg("init").current_dir(tmp.path())`; assert exit status is 0; assert `tmp.path().join("envy.toml")` exists; assert the file contains a valid UUID in the `project_id` field
- [x] T029 [US2] Write integration test `cli_set_and_get_round_trip` in `tests/cli_integration.rs`: initialise a temp project (`envy init`); run `envy set API_KEY=secret123`; run `envy get API_KEY` and capture stdout; assert stdout equals `"secret123\n"` exactly (no labels, no extra whitespace)
- [x] T030 [US2] Write integration test `cli_list_never_shows_values` in `tests/cli_integration.rs`: initialise a temp project; run `envy set API_KEY=secret123`; run `envy list` and capture stdout; assert stdout contains the string `"API_KEY"` and does NOT contain the string `"secret123"` anywhere
- [x] T031 [US2] Write integration test `cli_rm_then_get_fails` in `tests/cli_integration.rs`: initialise a temp project; run `envy set DEL_KEY=val`; run `envy rm DEL_KEY`; assert exit code 0; run `envy get DEL_KEY`; assert that final `get` exits with non-zero code
- [x] T032 [US3] Write integration test `cli_run_injects_secrets` in `tests/cli_integration.rs`: initialise a temp project; run `envy set ENVY_TEST_VAR=hello`; run `envy run -- printenv ENVY_TEST_VAR` and capture stdout; assert stdout equals `"hello\n"`
- [x] T033 [US3] Write integration test `cli_run_proxies_exit_code` in `tests/cli_integration.rs`: initialise a temp project (run needs vault open); run `envy run -- sh -c 'exit 42'`; assert the `envy` process exits with code 42
- [x] T034 [US4] Write integration test `cli_migrate_imports_env_file` in `tests/cli_integration.rs`: initialise a temp project; write a temp `.env` file containing 3 valid `KEY=VALUE` pairs, 1 comment line, and 1 blank line; run `envy migrate <path_to_file>`; assert exit code 0; verify all 3 keys are retrievable via `envy get` (run it 3 times, assert each stdout matches the expected value)
- [x] T035 Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml` if not already present (needed by integration tests); run `cargo test --no-run` to confirm `tests/cli_integration.rs` compiles; note that tests will panic at runtime until T036 is complete

### Dispatch implementation (T036)

- [x] T036 Implement `pub fn run() -> i32` in `src/cli/mod.rs` replacing the `todo!()` stub with full dispatch logic: (1) `let cli = Cli::parse()`; (2) match `Commands::Init` → call `commands::cmd_init()`, map `Ok(())` → return 0, map `Err(e)` → `eprintln!("{}", format_cli_error(&e)); return cli_exit_code(&e)`; (3) for all other commands extract `env` as `cli.command.env().as_deref().unwrap_or("")`; resolve `project_id` and open vault: call `crate::core::find_manifest(&std::env::current_dir().unwrap())` mapping `Err(e)` → `eprintln!("{}", format_core_error(&e)); return core_exit_code(&e)`; call `crate::crypto::get_or_create_master_key(&manifest.project_id)` mapping `Err(e)` → `eprintln!("error: {}", e); return 4`; call `crate::db::Vault::open(&vault_path(), master_key.as_ref())` mapping `Err(e)` → `eprintln!("{}", format_cli_error(&CliError::VaultOpen(e.to_string()))); return 4`; extract `project_id = crate::db::ProjectId(manifest.project_id.clone())`; (4) dispatch each command variant to its `commands::cmd_*` function; for `Commands::Run { command, .. }` return `commands::cmd_run(&vault, master_key.as_ref(), &project_id, env, &command)` directly; for `Commands::Set { assignment, .. }` split `assignment.split_once('=')` here — on `None` return `cli_exit_code` for `InvalidAssignment`; for all `Result`-returning handlers map `Err(e)` to `eprintln + exit_code`; for `Ok(())` return 0; add all required `use` declarations; run `cargo build` and verify it compiles

**Checkpoint**: `cargo build` succeeds. All 7 commands are wired.

---

## Phase 6: Polish

**Purpose**: Quality gates — linting, formatting, full test suite, documentation update.

- [x] T037 [P] Run `cargo clippy -- -D warnings` and fix ALL warnings in `src/cli/` (unused imports, needless borrows, missing `?` operators, etc.)
- [x] T038 [P] Run `cargo fmt -- --check`; if it reports differences run `cargo fmt` and re-verify; ensure all files in `src/cli/` and `tests/` are formatted
- [x] T039 Run the full test suite `cargo test` — all tests (features 001 + 002 + 003 + 004) must pass with zero failures; integration tests that require a real OS keyring may be annotated `#[ignore]` if the test environment lacks one, but must be documented
- [x] T040 [P] Update `CLAUDE.md` — add `dirs = "5"` (home directory resolution) to the Active Technologies line; add a `004-cli-interface` entry to Recent Changes describing the 7 CLI commands, `CliError` enum, and dispatch logic

**Checkpoint**: All 4 polish tasks pass. Feature 004 is complete.

---

## Dependencies & Execution Order

```
T001 → T002 → T003 → T004    (setup — sequential, each builds on prior)
                    ↓
       T005 → T006 → T007    (CliError — sequential within error.rs)
                    ↓
T008–T014 (unit tests — write all, then...)
       ↓
T015  (compile verify)
       ↓
T016 → T017 → T018 → T019 → T020   (core handlers — sequential within commands.rs)
                                    ↓
                              T021  (test run)
                                    ↓
                   T022–T023  (complex handler tests — write together)
                         ↓
                         T024 (compile verify)
                              ↓
                   T025 → T026     (complex handler impls)
                              ↓
                         T027 (test run)
                                   ↓
        T028–T034 (integration tests — can be written together [P])
                   ↓
                   T035 (compile verify)
                         ↓
                         T036 (dispatch wiring)
                                ↓
           T037 [P]  T038 [P]  T039  T040 [P]
```

### Phase Dependencies

- **Phase 1** (T001–T004): No dependencies — start immediately
- **Phase 2** (T005–T007): Requires Phase 1 — error types used in handler tests
- **Phase 3** (T008–T021): Requires Phase 2 — tests import `CliError`, `CoreError`, exit-code fns
- **Phase 4** (T022–T027): Requires Phase 2 — shares `commands.rs` with Phase 3; can start after T021
- **Phase 5** (T028–T036): Requires Phases 3 and 4 — integration tests invoke `cmd_init` and other handlers
- **Phase 6** (T037–T040): Requires Phase 5 complete

### Parallel Opportunities

- T028–T034 (integration tests) can all be written in a single pass before T036
- T037, T038, T040 (clippy, fmt, CLAUDE.md) are independent and can run in any order
- T008–T011 (parsing unit tests) and T012–T014 (error unit tests) can be written in a single pass

---

## Implementation Strategy

### Single-developer sequence (recommended)

1. Phase 1 → 2 → 3 (tests then implementation) → 4 (tests then implementation) → 5 (tests then dispatch) → 6
2. Run `cargo test` after T021 and after T027 as interim checkpoints
3. Run `cargo test` after T036 to verify integration tests pass end-to-end
4. Phase 6 is the final gate before marking the feature complete

### Test helpers note

Integration tests (T028–T034) each require an initialised Envy project. Create a shared helper function in `tests/cli_integration.rs`:

```rust
fn setup_project(dir: &std::path::Path) {
    std::process::Command::new(env!("CARGO_BIN_EXE_envy"))
        .arg("init")
        .current_dir(dir)
        .status()
        .expect("envy init failed");
}
```

This eliminates duplication across the 6 tests that need an initialised project.

### Keyring note

Integration tests invoke `envy init` which calls `crypto::get_or_create_master_key` via the OS credential store. On CI environments without a keyring daemon (headless Linux), annotate these tests with `#[ignore]` and run them only in environments with a keyring available. Document this in the test file with a comment explaining the requirement.
