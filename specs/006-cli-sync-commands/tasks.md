# Tasks: CLI Sync Commands (encrypt / decrypt)

**Input**: Design documents from `/specs/006-cli-sync-commands/`
**Prerequisites**: spec.md ✓ | plan.md ✓ | research.md ✓ | data-model.md ✓ | contracts/cli-sync.md ✓

**Approach**: Strict TDD — every test task MUST compile before its implementation task begins. Tests MUST fail before implementation. Tests MUST pass after.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel with other [P] tasks in the same phase
- **[US1–US4]**: Maps to User Story in spec.md

---

## Phase 1: Setup

**Purpose**: Add the new dependency, scaffold the new `CliError` variants and `Commands` variants so all subsequent phases compile cleanly from the start.

- [x] T001 Add `dialoguer = "0.11"` to `[dependencies]` in `Cargo.toml`
- [x] T002 Add `PassphraseInput(String)` variant to `CliError` enum in `src/cli/error.rs`
- [x] T003 Add `NothingImported` variant to `CliError` enum in `src/cli/error.rs`
- [x] T004 Add `CliError::PassphraseInput(_) => 2` arm to `cli_exit_code()` in `src/cli/error.rs`
- [x] T005 Add `CliError::NothingImported => 1` arm to `cli_exit_code()` in `src/cli/error.rs`
- [x] T006 Write unit test `passphrase_input_maps_to_exit_code_2` in `src/cli/error.rs` — assert `cli_exit_code(&CliError::PassphraseInput("x".into())) == 2` (contract test from `contracts/cli-sync.md`)
- [x] T007 Write unit test `nothing_imported_maps_to_exit_code_1` in `src/cli/error.rs` — assert `cli_exit_code(&CliError::NothingImported) == 1` (contract test from `contracts/cli-sync.md`)
- [x] T008 Add `Encrypt { env: Option<String> }` variant (doc comment + `#[command(alias = "enc")]`, `-e/--env` arg) and `Decrypt` variant (doc comment + `#[command(alias = "dec")]`) to `Commands` enum in `src/cli/mod.rs`
- [x] T009 Add private `fn artifact_path(manifest_path: &std::path::Path) -> std::path::PathBuf` helper to `src/cli/mod.rs` — returns `manifest_path.parent().expect("…").join("envy.enc")`
- [x] T010 Change `let (manifest, _)` to `let (manifest, manifest_path)` in `run()` in `src/cli/mod.rs`, then add stub dispatch arms for `Commands::Encrypt { env }` and `Commands::Decrypt` that each call `todo!()` (ensures the enum is exhaustive and the codebase compiles)
- [x] T011 Run `cargo test --no-run` to verify Phase 1 compiles cleanly with zero errors

**Checkpoint**: `CliError` and `Commands` scaffolding is complete. All error exit-code tests (T006, T007) pass. The codebase compiles. Phase 2 can begin.

---

## Phase 2: TDD — `cmd_encrypt` (US1 + US4 headless encrypt)

**Goal**: `envy encrypt` / `envy enc` seals the vault into `envy.enc`; accepts passphrase from terminal or `ENVY_PASSPHRASE` env var.

**Independent Test**: Set `ENVY_PASSPHRASE`, call `cmd_encrypt` against a tempfile vault that has one secret, verify `envy.enc` is created and readable.

### Tests for `cmd_encrypt` (write first — must compile, must fail)

- [x] T012 [US1] Write unit test `encrypt_writes_envy_enc_with_correct_environments` in `src/cli/commands.rs` — opens a tempfile `Vault`, writes one secret to `development`, sets `ENVY_PASSPHRASE=test-pass`, calls `cmd_encrypt(…, artifact_path, None)`, asserts `artifact_path.exists()` and the JSON contains `"development"` but NOT the secret value in plaintext (contract: `cmd_encrypt` writes `envy.enc` with correct environments)
- [x] T013 [US4] Write unit test `encrypt_uses_envy_passphrase_env_var_no_prompt` in `src/cli/commands.rs` — sets `ENVY_PASSPHRASE=headless-pass`, calls `cmd_encrypt`, asserts `Ok(())` is returned without any terminal interaction (contract: `cmd_encrypt` uses `ENVY_PASSPHRASE` when set)
- [x] T014 [US1] Write unit test `encrypt_empty_envy_passphrase_returns_passphrase_input_error` in `src/cli/commands.rs` — sets `ENVY_PASSPHRASE=   ` (whitespace only), calls `cmd_encrypt`, asserts `Err(CliError::PassphraseInput(_))` is returned (exit code 2 path)
- [x] T015 [US1] Write unit test `encrypt_env_filter_seals_only_named_environment` in `src/cli/commands.rs` — vault with secrets in `development` and `staging`, passes `env_filter = Some("staging")`, asserts resulting `envy.enc` contains `"staging"` but not `"development"`
- [x] T016 Run `cargo test --no-run` — verify all Phase 2 tests compile (they will fail at runtime because `cmd_encrypt` is not yet implemented)

### Implementation of `cmd_encrypt`

- [x] T017 [US1] Implement the private passphrase resolution helper `fn resolve_passphrase(prompt: &str, confirm: bool) -> Result<zeroize::Zeroizing<String>, crate::cli::CliError>` in `src/cli/commands.rs`:
  1. Check `std::env::var("ENVY_PASSPHRASE").ok().filter(|p| !p.trim().is_empty())` — if `Some(s)` return `Zeroizing::new(s)` immediately
  2. If `confirm == true`: use `dialoguer::Password::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt(prompt).with_confirmation("Confirm passphrase", "Passphrases do not match.").interact()` mapped to `CliError::PassphraseInput`
  3. If `confirm == false`: use `dialoguer::Password::with_theme(…).with_prompt(prompt).interact()` mapped to `CliError::PassphraseInput`
  4. Wrap result in `Zeroizing::new()`; validate `!passphrase.trim().is_empty()` or return `Err(CliError::PassphraseInput("passphrase must not be empty".into()))`
- [x] T018 [US1] Implement `pub(super) fn cmd_encrypt(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, artifact_path: &std::path::Path, env_filter: Option<&str>) -> Result<(), crate::cli::CliError>` in `src/cli/commands.rs`:
  1. Resolve passphrase via `resolve_passphrase("Enter passphrase", true)`
  2. Build `envs_slice`: if `env_filter.is_some()` wrap in a vec and pass `Some(&[env_filter.unwrap()])`, else pass `None`
  3. Call `crate::core::seal_artifact(vault, master_key, project_id, passphrase.as_ref(), envs)` — map `SyncError` to `CliError::VaultOpen` (or a new mapping)
  4. Call `crate::core::write_artifact(&artifact, artifact_path)` — map `SyncError::Io` to `CliError::VaultOpen`
  5. Print success header: `println!("Sealed {} environment(s) → {}", artifact.environments.len(), artifact_path.display())`
  6. For each `env_name` in `artifact.environments.keys()`: `println!("  {}  {}", console::style("✓").green(), env_name)`
- [x] T019 [US1] Replace `todo!()` stub for `Commands::Encrypt { env }` in `run()` in `src/cli/mod.rs` with the real dispatch: `let artifact_path = artifact_path(&manifest_path); let env_filter = env.as_deref(); match commands::cmd_encrypt(&vault, &master_key, &project_id, &artifact_path, env_filter) { Ok(()) => 0, Err(e) => { eprintln!("error: {e}"); cli_exit_code(&e) } }`
- [x] T020 Run `cargo test` — verify all Phase 2 tests pass (T012–T015) and no regressions in existing suite

**Checkpoint**: `envy encrypt` / `envy enc` fully functional. Artifact written to `<manifest_dir>/envy.enc`. ENVY_PASSPHRASE headless mode works.

---

## Phase 3: TDD — `cmd_decrypt` (US2 + US3 Progressive Disclosure + US4 headless decrypt)

**Goal**: `envy decrypt` / `envy dec` reads `envy.enc`, unseals it, upserts secrets into the vault; skipped environments are displayed without exiting non-zero.

**Independent Test**: Pre-seal an artifact with a known passphrase, call `cmd_decrypt` with `ENVY_PASSPHRASE` set, verify vault contains expected secret values.

### Tests for `cmd_decrypt` (write first — must compile, must fail)

- [x] T021 [US2] Write unit test `decrypt_imports_all_secrets_with_correct_passphrase` in `src/cli/commands.rs` — pre-seal an artifact (using `crate::core::seal_artifact`) with `ENVY_PASSPHRASE=pass` containing `development/API_KEY=sk_test`, call `cmd_decrypt`, then call `crate::core::get_secret` on the vault and assert `"sk_test"` is returned (contract: `cmd_decrypt` imports all secrets with correct passphrase)
- [x] T022 [US2] Write unit test `decrypt_returns_nothing_imported_when_all_envs_skipped` in `src/cli/commands.rs` — seal artifact with `passphrase-a`, set `ENVY_PASSPHRASE=wrong-passphrase`, call `cmd_decrypt`, assert `Err(CliError::NothingImported)` (contract: `cmd_decrypt` returns `NothingImported` when all envs skipped)
- [x] T023 [US3] Write unit test `decrypt_exits_ok_and_shows_skipped_for_partial_access` in `src/cli/commands.rs` — build `SyncArtifact` manually with `development` sealed with `dev-pass` and `production` sealed with `prod-pass` (use `crate::core::seal_envelope` directly), write to tempfile, set `ENVY_PASSPHRASE=dev-pass`, call `cmd_decrypt`, assert `Ok(())` is returned and the vault contains `development` secrets but not `production` secrets (contract: `cmd_decrypt` exits 0 for partial access)
- [x] T024 [US2] Write unit test `decrypt_returns_error_when_envy_enc_not_found` in `src/cli/commands.rs` — pass a nonexistent `artifact_path`, assert the returned error displays `"not found"` and maps to exit code 1 (contract: Exit code 1 when `envy.enc` not found)
- [x] T025 [US2] Write unit test `decrypt_returns_error_for_malformed_envy_enc` in `src/cli/commands.rs` — write `b"this is not json"` to a tempfile path, call `cmd_decrypt`, assert error maps to exit code 4 (contract: Exit code 4 for malformed `envy.enc`)
- [x] T026 [US2] Write unit test `decrypt_returns_passphrase_input_error_for_empty_passphrase` in `src/cli/commands.rs` — seal a valid artifact, set `ENVY_PASSPHRASE=   ` (whitespace), call `cmd_decrypt`, assert `Err(CliError::PassphraseInput(_))` and exit code 2 (contract: Exit code 2 for empty passphrase)
- [x] T027 Run `cargo test --no-run` — verify all Phase 3 tests compile (they will fail at runtime because `cmd_decrypt` is not yet implemented)

### Implementation of `cmd_decrypt`

- [x] T028 [US2] Implement `pub(super) fn cmd_decrypt(vault: &crate::db::Vault, master_key: &[u8; 32], project_id: &crate::db::ProjectId, artifact_path: &std::path::Path) -> Result<(), crate::cli::CliError>` in `src/cli/commands.rs`:
  1. Call `crate::core::read_artifact(artifact_path)` — map `SyncError::FileNotFound` to a `CliError` that displays `"envy.enc not found"` with exit code 1; map `SyncError::Artifact(MalformedArtifact)` and `SyncError::UnsupportedVersion` to display errors with exit code 4
  2. Resolve passphrase via `resolve_passphrase("Enter passphrase", false)` (single-entry, no confirm)
  3. Call `crate::core::unseal_artifact(&artifact, passphrase.as_ref())` — map `SyncError::Artifact(WeakPassphrase)` to `CliError::PassphraseInput`
  4. If `result.imported.is_empty()` → return `Err(CliError::NothingImported)`
  5. For each `(env_name, secrets)` in `result.imported.iter()`: for each `(key, value)` call `crate::core::set_secret(vault, master_key, project_id, env_name, key, value.as_ref())` — on error print warning to stderr and continue
  6. Print header: `println!("Imported {} environment(s) from envy.enc", result.imported.len())`
  7. For each `env_name` in `result.imported.keys()`: `println!("  {}  {} ({} secret(s) upserted)", console::style("✓").green(), env_name, secret_count)`
- [x] T029 [US3] Implement coloured skipped-environment output in `cmd_decrypt` (add after success lines): for each `env_name` in `result.skipped.iter()`: `println!("  {}  {} skipped \u{2014} different passphrase or key", console::style("⚠").yellow().dim(), env_name)` — exit code remains 0 (Progressive Disclosure contract)
- [x] T030 [US4] Verify `cmd_decrypt` passphrase resolution reuses `resolve_passphrase("Enter passphrase", false)` — confirm `ENVY_PASSPHRASE` is checked before any terminal interaction (same helper as `cmd_encrypt`)
- [x] T031 [US2] Replace `todo!()` stub for `Commands::Decrypt` in `run()` in `src/cli/mod.rs` with real dispatch: `let artifact_path = artifact_path(&manifest_path); match commands::cmd_decrypt(&vault, &master_key, &project_id, &artifact_path) { Ok(()) => 0, Err(e) => { eprintln!("error: {e}"); cli_exit_code(&e) } }`
- [x] T032 Run `cargo test` — verify all Phase 3 tests pass (T021–T026) and no regressions in existing suite

**Checkpoint**: `envy decrypt` / `envy dec` fully functional. Progressive Disclosure output is correct (green ✓ for imported, yellow ⚠ for skipped, exit code 0). NothingImported exits 1. ENVY_PASSPHRASE headless mode works.

---

## Phase 4: Integration Tests (US1 + US2 alias validation)

**Goal**: Two ignored integration tests in `tests/cli_integration.rs` confirming the `enc` and `dec` aliases are wired correctly end-to-end. These tests require the OS keyring and are skipped in CI.

- [x] T033 [US1] Add ignored integration test `cli_encrypt_and_enc_alias_work` to `tests/cli_integration.rs` with `#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]` — tests that running `envy enc` (with `ENVY_PASSPHRASE` set) in an initialised project directory produces `envy.enc` (contract: `envy encrypt` / `envy enc` alias works)
- [x] T034 [US2] Add ignored integration test `cli_decrypt_and_dec_alias_work` to `tests/cli_integration.rs` with `#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]` — tests that running `envy dec` (with `ENVY_PASSPHRASE` set) against a pre-written `envy.enc` imports secrets into the vault (contract: `envy decrypt` / `envy dec` alias works)
- [x] T035 Run `cargo test` — verify the two new integration tests appear in output as ignored and all other tests still pass

**Checkpoint**: All 12 contract test cases from `contracts/cli-sync.md` have dedicated tasks and corresponding tests. Integration stubs are in place.

---

## Phase 5: Polish

**Purpose**: Enforce code quality standards and record the feature completion.

- [x] T036 Run `cargo clippy -- -D warnings` and fix any warnings in `Cargo.toml`, `src/cli/mod.rs`, `src/cli/commands.rs`, and `src/cli/error.rs`
- [x] T037 Run `cargo fmt` to apply standard Rust formatting across all modified files
- [x] T038 Run `cargo test` (full suite — all unit + integration tests) and confirm the baseline passes with 0 failures (integration tests expected to appear as ignored)
- [x] T039 Update `CLAUDE.md` to record `dialoguer = "0.11"` under Active Technologies and add `006-cli-sync-commands: Added dialoguer = "0.11"; implemented encrypt/enc and decrypt/dec commands with ENVY_PASSPHRASE CI/CD headless mode and Progressive Disclosure coloured output` under Recent Changes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (cmd_encrypt)**: Requires Phase 1 complete (CliError variants, Commands enum, compile check)
- **Phase 3 (cmd_decrypt)**: Requires Phase 2 complete (shares `resolve_passphrase` helper)
- **Phase 4 (Integration Tests)**: Requires Phase 3 complete (both commands must be functional for alias tests)
- **Phase 5 (Polish)**: Requires Phase 4 complete

### Contract Test Coverage Map

Every test required by `contracts/cli-sync.md` is assigned a specific task:

| Contract Test | Task |
|---|---|
| `cmd_encrypt` writes `envy.enc` with correct environments | T012 |
| `cmd_encrypt` uses `ENVY_PASSPHRASE` when set (no prompt) | T013 |
| `cmd_decrypt` imports all secrets with correct passphrase | T021 |
| `cmd_decrypt` returns `NothingImported` when all envs skipped | T022 |
| `cmd_decrypt` exits 0 and shows skipped for partial access | T023 |
| `envy encrypt` / `envy enc` alias works | T033 |
| `envy decrypt` / `envy dec` alias works | T034 |
| Exit code 1 when `envy.enc` not found | T024 |
| Exit code 2 for empty passphrase | T026 |
| Exit code 4 for malformed `envy.enc` | T025 |
| `PassphraseInput` maps to exit code 2 | T006 |
| `NothingImported` maps to exit code 1 | T007 |

### Parallel Opportunities

- T002 and T003 (both add `CliError` variants) can be written as one edit, or done in parallel with T008 (Commands enum) since they touch different files.
- T012–T015 (all `cmd_encrypt` test tasks) can be written in parallel — they are all in `src/cli/commands.rs` test module but have no dependency on each other.
- T021–T026 (all `cmd_decrypt` test tasks) can be written in parallel.
- T036 (clippy) and T037 (fmt) must be run sequentially (fmt changes must not reintroduce clippy issues).

---

## Implementation Strategy

### MVP (Phase 1 + Phase 2 only)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (TDD `cmd_encrypt`)
3. **STOP**: `envy encrypt` is fully working. Team can seal and commit `envy.enc`. This is independently valuable even before `decrypt` is available.

### Full Delivery (all phases)

1. Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
2. Each phase ends with a `cargo test` checkpoint
3. No phase begins until the previous checkpoint passes

---

## Notes

- **`ENVY_PASSPHRASE` in tests**: Use `std::env::set_var("ENVY_PASSPHRASE", "…")` + `std::env::remove_var("ENVY_PASSPHRASE")` in test teardown (or use a scoped guard pattern) to avoid polluting other tests. Tests that rely on `ENVY_PASSPHRASE` MUST unset it before returning, even on failure.
- **Argon2id KDF in tests**: The production KDF params (64 MiB × 3) run for ~1.5s per environment. Unit tests for `cmd_encrypt`/`cmd_decrypt` that use `seal_artifact` will be slow. This is acceptable for an LLM-executed TDD loop; no test-speed shortcut is needed since these are integration-level unit tests.
- **`console` crate colour suppression**: In unit tests, stdout is not a TTY so `console::style("✓").green()` renders as plain text with no ANSI codes. Tests should not assert on ANSI codes — only on the text content.
- **`resolve_passphrase` sharing**: The same helper is used by both `cmd_encrypt` (confirm=true) and `cmd_decrypt` (confirm=false). Implement it once in `src/cli/commands.rs` and call it from both handlers.
