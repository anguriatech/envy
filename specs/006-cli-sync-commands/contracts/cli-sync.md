# Contract: CLI Sync Commands (encrypt / decrypt)

**Feature**: 006-cli-sync-commands
**Date**: 2026-03-22
**Stability**: Draft — awaiting approval

This document extends the CLI contract from `specs/004-cli-interface/contracts/cli.md` with the two new GitOps synchronization subcommands.

---

## Layer Placement

```
src/cli/mod.rs       ← Commands enum: add Encrypt, Decrypt variants
                     ← run(): add dispatch arms
src/cli/commands.rs  ← cmd_encrypt, cmd_decrypt handlers (pub(super))
src/cli/error.rs     ← CliError: add PassphraseInput, NothingImported variants
```

All sync business logic remains in `crate::core::sync`. The CLI layer MUST NOT call crypto or DB functions directly (except `Vault::open` and `get_or_create_master_key`, which are the two permitted infrastructure exceptions established in feature 004).

---

## New Dependency

```toml
# Cargo.toml [dependencies]
dialoguer = "0.11"
# console = "0.15" — comes transitively via dialoguer; no explicit entry needed
```

---

## Commands Enum Extension

```rust
// src/cli/mod.rs — added to Commands enum

/// Seal the local vault into an encrypted `envy.enc` artifact (alias: enc).
///
/// All environments are sealed by default. Use `-e` to seal a single environment.
/// Prompts for a passphrase with confirmation (or reads ENVY_PASSPHRASE in CI).
#[command(alias = "enc")]
Encrypt {
    /// Seal only this environment (default: all environments in the vault).
    #[arg(short = 'e', long = "env", value_name = "ENV")]
    env: Option<String>,
},

/// Unseal `envy.enc` and upsert secrets into the local vault (alias: dec).
///
/// Successfully decrypted environments are upserted. Environments that cannot
/// be decrypted with the provided passphrase are listed as skipped (not an error).
/// Exits non-zero only if zero environments are imported.
#[command(alias = "dec")]
Decrypt,
```

---

## Handler Function Signatures

```rust
// src/cli/commands.rs — both handlers are pub(super)

/// Reads all secrets from the vault and writes a sealed `envy.enc` to
/// `artifact_path` (always `<manifest_dir>/envy.enc`).
///
/// Passphrase is obtained by (in priority order):
///   1. `ENVY_PASSPHRASE` env var (if non-empty) — headless/CI mode.
///   2. Interactive double-entry terminal prompt via dialoguer.
///
/// # Errors
/// - `CliError::PassphraseInput` if the terminal prompt fails or passphrase is empty.
/// - `SyncError::*` variants (propagated via `CliError` mapping) on seal/write failure.
pub(super) fn cmd_encrypt(
    vault:         &crate::db::Vault,
    master_key:    &[u8; 32],
    project_id:    &crate::db::ProjectId,
    artifact_path: &std::path::Path,
    env_filter:    Option<&str>,   // None = seal all environments
) -> Result<(), SyncCliError>;

/// Reads `artifact_path` (`<manifest_dir>/envy.enc`), unseals it, and upserts
/// all successfully decrypted secrets into the vault.
///
/// Passphrase is obtained by (in priority order):
///   1. `ENVY_PASSPHRASE` env var (if non-empty) — headless/CI mode.
///   2. Interactive single-entry terminal prompt via dialoguer.
///
/// # Errors
/// - `CliError::PassphraseInput` if the terminal prompt fails or passphrase is empty.
/// - `CliError::NothingImported` if `result.imported` is empty after unseal.
/// - `SyncError::*` variants on read/unseal failure.
pub(super) fn cmd_decrypt(
    vault:         &crate::db::Vault,
    master_key:    &[u8; 32],
    project_id:    &crate::db::ProjectId,
    artifact_path: &std::path::Path,
) -> Result<(), SyncCliError>;
```

> **Note on return type**: A local `SyncCliError` type alias is used to avoid clutter. Internally these functions return `Result<(), Box<dyn std::error::Error>>` or match into `CliError` / `SyncError` variants. The exact mechanism is an implementation detail; what matters is that the `run()` dispatcher maps them to the exit codes below.

---

## New `CliError` Variants

```rust
// src/cli/error.rs — added to CliError enum

/// The terminal passphrase prompt failed (IO error, Ctrl-C, or TTY not available).
#[error("passphrase input failed: {0}")]
PassphraseInput(String),

/// `decrypt` completed but zero environments could be decrypted.
/// The user should check their passphrase and retry.
#[error("no environments could be decrypted \u{2014} check your passphrase")]
NothingImported,
```

---

## Updated Exit-Code Table

Extends the table from `specs/004-cli-interface/contracts/cli.md`:

| Exit Code | Condition | Command |
|-----------|-----------|---------|
| 0 | Success; or partial import (≥1 env imported, some skipped) | `encrypt`, `decrypt` |
| 1 | `envy.enc` not found; zero environments imported | `decrypt` |
| 2 | Passphrase input error; empty or whitespace passphrase | `encrypt`, `decrypt` |
| 4 | Malformed `envy.enc`; unsupported version; vault error | `encrypt`, `decrypt` |

```rust
// Additions to cli_exit_code() in src/cli/error.rs
CliError::PassphraseInput(_) => 2,
CliError::NothingImported    => 1,
```

---

## Passphrase Resolution Contract

Both handlers MUST follow this exact lookup order, checked before any terminal interaction:

```
1. val = std::env::var("ENVY_PASSPHRASE").ok()
2. If val is Some(s) AND !s.trim().is_empty() → use s (headless mode)
3. Else if stdin is a TTY → prompt interactively via dialoguer
4. Else → return Err(CliError::PassphraseInput("not a terminal and ENVY_PASSPHRASE not set"))
```

**Invariants**:
- The resolved passphrase MUST be wrapped in `Zeroizing::new()` immediately.
- The passphrase MUST NOT be printed, logged, or included in any error message.
- An empty or whitespace-only `ENVY_PASSPHRASE` is treated as "not set" — falls through to terminal prompt.

---

## stdout / stderr Output Contract

### `cmd_encrypt` — success output

Written to **stdout**. One header line, then one line per sealed environment, then a footer.

```
Sealed {N} environment(s) → envy.enc
  ✓  development   (3 secrets)
  ✓  staging       (2 secrets)
```

- Header and `✓` lines: plain text (no colour required, but green `✓` preferred when TTY).
- If zero environments exist in vault: header shows `0` and no environment lines.
- Overwrites any existing `envy.enc` without asking.

### `cmd_decrypt` — success output (all imported)

Written to **stdout**.

```
Imported {N} environment(s) from envy.enc
  ✓  development   (3 secrets upserted)
  ✓  production    (1 secret upserted)
```

### `cmd_decrypt` — partial success (Progressive Disclosure)

```
Imported {N} environment(s) from envy.enc
  ✓  development   (3 secrets upserted)
  ⚠  production    skipped — different passphrase or key
```

- `✓` lines: green when TTY.
- `⚠` lines: yellow+dim when TTY. MUST be written to **stdout** (not stderr), because they are informational — not errors. Exit code is **0**.

### `cmd_decrypt` — nothing imported (error)

Written to **stderr** by the `run()` dispatcher (same pattern as all other errors):

```
error: no environments could be decrypted — check your passphrase
```

Exit code: **1**.

---

## Dispatch Arm in `run()` (src/cli/mod.rs)

```rust
// Pattern to add inside the match cli.command { ... } block in run()

Commands::Encrypt { env } => {
    let artifact_path = artifact_path(&manifest_path);
    let env_filter = env.as_deref();
    match commands::cmd_encrypt(&vault, &master_key, &project_id, &artifact_path, env_filter) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e.to_string());
            e.exit_code()
        }
    }
}

Commands::Decrypt => {
    let artifact_path = artifact_path(&manifest_path);
    match commands::cmd_decrypt(&vault, &master_key, &project_id, &artifact_path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e.to_string());
            e.exit_code()
        }
    }
}
```

Helper (added to `src/cli/mod.rs`):

```rust
/// Returns the canonical path for the `envy.enc` artifact.
/// Always co-located with `envy.toml` in the project root.
fn artifact_path(manifest_path: &std::path::Path) -> std::path::PathBuf {
    manifest_path
        .parent()
        // Safety: manifest_path is a file returned by find_manifest;
        // it is never "/" so parent() is always Some.
        .expect("manifest path must have a parent directory")
        .join("envy.enc")
}
```

> **Note**: `manifest_path` is the path to the `envy.toml` file itself (returned as the second element of the `find_manifest` result tuple). The dispatch pattern for `Init` bypasses vault/manifest setup; `Encrypt` and `Decrypt` use the standard path (same as `Set`, `Get`, etc.).

---

## Security Invariants

| Invariant | Enforced by |
|-----------|-------------|
| Passphrase wrapped in `Zeroizing<String>` immediately | `cmd_encrypt`, `cmd_decrypt` before any call |
| Passphrase never appears in log output or error strings | Convention + review gate |
| Vault not modified on decrypt if zero envs imported | `cmd_decrypt`: check `imported.is_empty()` before `set_secret` loop |
| No partial vault state on multi-env import failure | Each `set_secret` is independent; individual failures logged as warning, import continues |
| Coloured output suppressed when not a TTY | `console` crate handles automatically |
| `ENVY_PASSPHRASE` env var trimmed before empty check | Both handlers: `.filter(|p| !p.trim().is_empty())` |

---

## Test Requirements

All new code MUST have corresponding tests:

| Test | Location |
|------|----------|
| `cmd_encrypt` writes `envy.enc` with correct environments | `src/cli/commands.rs` unit test (tempfile) |
| `cmd_encrypt` uses `ENVY_PASSPHRASE` when set (no prompt) | `src/cli/commands.rs` unit test (env var mock) |
| `cmd_decrypt` imports all secrets with correct passphrase | `src/cli/commands.rs` unit test (tempfile + pre-sealed artifact) |
| `cmd_decrypt` returns `NothingImported` when all envs skipped | `src/cli/commands.rs` unit test |
| `cmd_decrypt` exits 0 and prints skipped line for partial access | `src/cli/commands.rs` unit test |
| `envy encrypt` / `envy enc` alias works | `tests/cli_integration.rs` (skipped without keyring) |
| `envy decrypt` / `envy dec` alias works | `tests/cli_integration.rs` (skipped without keyring) |
| Exit code 1 when `envy.enc` not found | `src/cli/commands.rs` unit test |
| Exit code 2 for empty passphrase | `src/cli/commands.rs` unit test |
| Exit code 4 for malformed `envy.enc` | `src/cli/commands.rs` unit test |
| `PassphraseInput` maps to exit code 2 | `src/cli/error.rs` unit test |
| `NothingImported` maps to exit code 1 | `src/cli/error.rs` unit test |
