# Implementation Plan: CLI Sync Commands (encrypt / decrypt)

**Branch**: `006-cli-sync-commands` | **Date**: 2026-03-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/006-cli-sync-commands/spec.md`

---

## Summary

Add two new `clap` subcommands — `encrypt` (alias `enc`) and `decrypt` (alias `dec`) — to the existing `src/cli/` module. These commands are the user-facing interface for the GitOps sync artifact pipeline built in feature 005. They read a passphrase (interactively or from `ENVY_PASSPHRASE`), delegate to `crate::core::sync`, and format the output with coloured success/skip feedback.

**New dependency**: `dialoguer = "0.11"` (provides hidden passphrase prompt with double-entry confirmation; the `console` crate for colour comes transitively).

No new database entities, no new modules, no changes to the core or crypto layers.

---

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.85
**Primary Dependencies**:
- `dialoguer = "0.11"` (new — passphrase input + colour via transitive `console`)
- `clap` (existing — derive API; extend `Commands` enum)
- `crate::core::sync` (existing — `seal_artifact`, `unseal_artifact`, `write_artifact`, `read_artifact`)
- `crate::core::{find_manifest, set_secret}` (existing)
- `crate::crypto::get_or_create_master_key` (existing)
- `crate::db::Vault` (existing)

**Storage**: No changes — `envy.enc` written by `write_artifact` in the Core layer; vault written by `set_secret` in the Core layer.

**Testing**: `cargo test` — unit tests in `src/cli/commands.rs` and `src/cli/error.rs`; integration tests in `tests/cli_integration.rs` (OS-keyring tests skipped in CI).

**Target Platform**: Linux, macOS, Windows (same as existing CLI).

**Project Type**: CLI binary extension.

**Performance Goals**: `encrypt` and `decrypt` complete in < 5 seconds for typical vaults (Argon2id KDF is the bottleneck at ~0.5s/env; acceptable for a manual sync operation).

**Constraints**: No new database migrations. No changes to the public `lib.rs` API surface. Must remain compatible with the exit-code table from feature 004.

---

## Constitution Check

*Pre-Phase-0 gate — all principles checked.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | ✅ Pass | Passphrase wrapped in `Zeroizing<String>` immediately after terminal/env read. Never logged. `UnsealResult` values are `Zeroizing<String>` from Core layer. |
| II. Determinism | ✅ Pass | Output format is fixed by contract. Colour is suppressed automatically when not a TTY (via `console` crate). No locale-dependent behaviour. |
| III. Rust Best Practices | ✅ Pass | All new code uses `Result<T, E>`. Single `.expect()` is justified in the `artifact_path` helper (parent always exists — documented). Unit tests required for all handlers and error variants. |
| IV. Modularity | ✅ Pass | CLI layer calls only `crate::core::sync::*` and `set_secret`. Does not import `crate::crypto` directly (except the two permitted exceptions: `Vault::open` and `get_or_create_master_key`, established in feature 004). |
| V. Language | ✅ Pass | All identifiers, comments, and output strings are in English. |

**No violations. No Complexity Tracking table required.**

*Post-Phase-1 gate: confirmed — design introduces no new cross-layer dependencies beyond those pre-approved in feature 004.*

---

## Project Structure

### Documentation (this feature)

```text
specs/006-cli-sync-commands/
├── plan.md              ← This file
├── spec.md              ← Feature specification
├── research.md          ← Tech decisions (crate selection, env var naming, etc.)
├── data-model.md        ← In-memory data flow + new CLI types
├── quickstart.md        ← Acceptance scenarios
├── contracts/
│   └── cli-sync.md     ← Handler signatures, output format, exit codes
└── checklists/
    └── requirements.md  ← Spec quality gate (12/12 pass)
```

### Source Code Changes

```text
Cargo.toml
  └── + dialoguer = "0.11"

src/cli/
  ├── mod.rs      ← Add: Encrypt{env}, Decrypt variants to Commands enum
  │               ← Add: dispatch arms in run()
  │               ← Add: artifact_path() helper
  ├── commands.rs ← Add: cmd_encrypt(), cmd_decrypt() (pub(super))
  └── error.rs    ← Add: PassphraseInput(String), NothingImported variants
                  ← Add: exit-code mappings for new variants

tests/
  └── cli_integration.rs  ← Add: two ignored integration test stubs
                             (require OS keyring; skipped in CI)
```

**No new files. No new modules. No changes outside `src/cli/`.**

---

## Algorithm Specifications

### `cmd_encrypt` algorithm

```
1. Resolve passphrase:
   a. Check std::env::var("ENVY_PASSPHRASE") — if Some(s) and !s.trim().is_empty():
      passphrase = Zeroizing::new(s)
   b. Else: prompt via dialoguer Password::with_theme(ColorfulTheme)
                                    .with_prompt("Enter passphrase")
                                    .with_confirmation("Confirm passphrase",
                                                       "Passphrases do not match.")
                                    .interact()
      On error → Err(CliError::PassphraseInput(e.to_string()))

2. Validate passphrase (trim check):
   If passphrase.trim().is_empty() → Err(CliError::PassphraseInput("passphrase must not be empty"))
   [Note: seal_artifact will also reject via ArtifactError::WeakPassphrase — defensive double-check]

3. Call seal_artifact(vault, master_key, project_id, passphrase.as_ref(), env_filter_slice)
   On error → map SyncError to CliError variant, return Err

4. Call write_artifact(&artifact, artifact_path)
   On error → Err(CliError wrapping SyncError::Io)

5. Print success output:
   println!("Sealed {} environment(s) → {}", count, artifact_path.display())
   For each (env_name, envelope) in artifact.environments.iter():
     let secret_count = /* count from vault */ — or infer from envelope (opaque)
     println!("  {}  {}", style("✓").green(), env_name)
```

> **Note on secret count in encrypt**: Because the envelope ciphertext is opaque after sealing, the count of secrets per environment is obtained by calling `crate::core::get_env_secrets` before sealing, or captured as metadata during the seal loop. The simplest approach: count the secrets per environment from the vault before calling `seal_artifact`, or change `seal_artifact` to return per-env counts. The exact mechanism is an implementation detail — the output SHOULD show counts if feasible; if not, the environment name alone is sufficient.

### `cmd_decrypt` algorithm

```
1. Read artifact: read_artifact(artifact_path)
   SyncError::FileNotFound → Err(CliError mapping) with exit 1
   SyncError::Artifact(MalformedArtifact) → Err with exit 4
   SyncError::UnsupportedVersion → Err with exit 4

2. Resolve passphrase (same lookup order as cmd_encrypt, but single-entry prompt):
   a. Check ENVY_PASSPHRASE env var → use if non-empty
   b. Else prompt via dialoguer Password::with_prompt("Enter passphrase").interact()
   On error → Err(CliError::PassphraseInput)

3. Call unseal_artifact(&artifact, passphrase.as_ref())
   SyncError::Artifact(WeakPassphrase) → Err(CliError::PassphraseInput) exit 2
   Other SyncError → Err mapped to exit 4

4. Check result.imported.is_empty():
   If true → Err(CliError::NothingImported) exit 1

5. Upsert loop — for each (env_name, secrets) in result.imported:
   For each (key, value) in secrets:
     crate::core::set_secret(vault, master_key, project_id, env_name, key, value.as_ref())
     On error: print warning to stderr, continue (do not abort full import)

6. Print success output:
   println!("Imported {} environment(s) from envy.enc", result.imported.len())
   For each env in result.imported.keys():
     println!("  {}  {}", style("✓").green(), env_name)
   For each env in result.skipped:
     println!("  {}  {} skipped — different passphrase or key",
              style("⚠").yellow().dim(), env_name)
```

---

## Error Mapping Table

| Error source | CliError variant | Exit code |
|---|---|---|
| `dialoguer::Error` (prompt IO) | `PassphraseInput(e.to_string())` | 2 |
| Empty passphrase | `PassphraseInput("passphrase must not be empty")` | 2 |
| `SyncError::FileNotFound` | map to `CliError` display with `"envy.enc not found"` | 1 |
| `result.imported.is_empty()` | `NothingImported` | 1 |
| `SyncError::Artifact(WeakPassphrase)` | `PassphraseInput(...)` | 2 |
| `SyncError::Artifact(MalformedArtifact)` | display via `format_sync_error` | 4 |
| `SyncError::UnsupportedVersion` | display via `format_sync_error` | 4 |
| `SyncError::Io` | display via `format_sync_error` | 4 |
| `SyncError::VaultError` | display via `format_sync_error` | 4 |

---

## Implementation Phases (for tasks.md)

### Phase 1: Setup
- Add `dialoguer = "0.11"` to `Cargo.toml`
- Add `Encrypt` and `Decrypt` variants to `Commands` enum in `src/cli/mod.rs`
- Add stub arms in `run()` dispatch that return `todo!()`
- Add `PassphraseInput` and `NothingImported` to `CliError`
- Add exit-code mappings for new variants

### Phase 2: TDD — `cmd_encrypt`
- Write unit tests for `cmd_encrypt` (compile-check first)
- Implement `cmd_encrypt` in `src/cli/commands.rs`
- Implement `artifact_path` helper in `src/cli/mod.rs`

### Phase 3: TDD — `cmd_decrypt`
- Write unit tests for `cmd_decrypt`
- Implement `cmd_decrypt` in `src/cli/commands.rs`
- Wire dispatch arms in `run()` (replace `todo!()` with real calls)

### Phase 4: Integration Tests
- Add two ignored integration test stubs in `tests/cli_integration.rs`

### Phase 5: Polish
- `cargo clippy -- -D warnings`
- `cargo fmt`
- `cargo test` (full suite)
- Update `CLAUDE.md`

---

## Complexity Tracking

*No violations — table omitted.*
