# Implementation Plan: 004-cli-interface

**Feature**: CLI Interface
**Branch**: `004-cli-interface`
**Date**: 2026-03-19
**Status**: Awaiting approval

---

## 1. Scope

Implement `src/cli/` as the user-facing entry point for Envy:

1. **Parse user intent** — clap Derive API structures map terminal arguments to typed Rust values.
2. **Own the Vault lifecycle** — the CLI is the sole layer that opens the vault and fetches the master key from the OS credential store; all other layers receive references.
3. **Dispatch to Core** — each subcommand calls one or more `core::*` functions with the vault reference and master key slice.
4. **Format output** — print decrypted values, key lists, and status messages to stdout; format errors as human-readable messages to stderr; proxy exit codes.

---

## 2. Architecture Position

```
src/
├── main.rs         ← calls envy::cli::run() and propagates exit code
├── cli/            ← THIS FEATURE
│   ├── mod.rs          Cli struct, Subcommand enum, dispatch, run()
│   ├── commands.rs     one handler fn per subcommand (7 fns)
│   └── error.rs        format_error() — CoreError → human string + exit code
├── core/           ← (complete) manifest + CRUD ops
├── crypto/         ← (complete) encrypt / decrypt / get_or_create_master_key
└── db/             ← (complete) Vault CRUD
```

**Dependency rule** (Constitution Principle IV):

```
cli → core → crypto   ✓
           → db       ✓
cli → db              ✗  (prohibited — CLI must not bypass Core)
cli → crypto          ✗  (prohibited — only for Vault::open key and master key retrieval)
core → cli            ✗  (prohibited)
```

> **Exception**: The CLI calls `crypto::get_or_create_master_key` (keyring) and passes the resulting `[u8; 32]` to `Vault::open` — this is the sole permitted direct crypto import in CLI.

---

## 3. New Dependencies

No new crates required. `clap` with `features = ["derive"]` is already declared in `Cargo.toml`.

---

## 4. File Structure

### 4.1 `src/cli/mod.rs` — Entry Point & Dispatch

Defines the clap command hierarchy and the top-level `run()` function that `main.rs` calls.

**`main.rs`** (modified):

```rust
use std::process;

fn main() {
    let code = envy::cli::run();
    process::exit(code);
}
```

**`mod.rs`** structure:

```rust
// Re-export so main.rs only needs one import.
mod commands;
mod error;

use clap::{Parser, Subcommand};

/// Envy — encrypted environment variable manager.
#[derive(Parser)]
#[command(name = "envy", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialise Envy in the current directory.
    Init,

    /// Store or update a secret (KEY=VALUE).
    Set {
        /// KEY=VALUE pair — value may contain additional `=` characters.
        assignment: String,
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },

    /// Print the decrypted value of a secret.
    Get {
        key: String,
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },

    /// List secret key names (never values). Alias: ls.
    #[command(alias = "ls")]
    List {
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },

    /// Delete a secret. Alias: remove.
    #[command(alias = "remove")]
    Rm {
        key: String,
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },

    /// Inject secrets as env vars and run a child process.
    Run {
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
        /// Command and arguments to execute (after `--`).
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Import secrets from a legacy .env file.
    Migrate {
        /// Path to the .env file.
        file: std::path::PathBuf,
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },
}

/// Top-level entry point called by `main.rs`.
/// Returns the process exit code (0 = success, non-zero = failure).
pub fn run() -> i32 { ... }
```

**`run()` algorithm**:

1. `Cli::parse()` — clap handles unknown flags and `--help` automatically.
2. For commands that need vault access (`set`, `get`, `list`, `rm`, `run`, `migrate`):
   a. `core::find_manifest(current_dir)` → `(manifest, _dir)`.
   b. `crypto::get_or_create_master_key(&manifest.project_id)` → `master_key: Zeroizing<[u8; 32]>`.
   c. `Vault::open(vault_path(), master_key.as_ref())` → `vault`.
3. Dispatch to the appropriate handler in `commands.rs`.
4. On `Ok(())` → return 0.
5. On `Err(e)` → `eprintln!("{}", error::format_error(e))` → return `error::exit_code(&e)`.

**Vault path helper** (private):

```rust
fn vault_path() -> PathBuf {
    // ~/.envy/vault.db
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".envy")
        .join("vault.db")
}
```

> `dirs` crate may be used for cross-platform home directory resolution. If not already in `Cargo.toml`, add `dirs = "5"`.

---

### 4.2 `src/cli/commands.rs` — Subcommand Handlers

One `pub(super) fn` per subcommand. Each function receives already-opened resources (`&Vault`, `&[u8; 32]`, `&ProjectId`) as parameters — no vault lifecycle management here.

```rust
// Signatures (all return Result<(), CoreError> except run which returns i32)

pub(super) fn cmd_init() -> Result<(), CliError>

pub(super) fn cmd_set(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    assignment: &str,
) -> Result<(), CoreError>

pub(super) fn cmd_get(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    key: &str,
) -> Result<(), CoreError>

pub(super) fn cmd_list(
    vault: &Vault,
    project_id: &ProjectId,
    env: &str,
) -> Result<(), CoreError>

pub(super) fn cmd_rm(
    vault: &Vault,
    project_id: &ProjectId,
    env: &str,
    key: &str,
) -> Result<(), CoreError>

/// Returns the child process exit code directly (not wrapped in Result).
pub(super) fn cmd_run(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    command: &[String],
) -> i32

pub(super) fn cmd_migrate(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    file: &Path,
) -> Result<(), CliError>
```

#### `cmd_init` design

`init` is the only command that does NOT need an existing manifest or open vault; it creates them.

Algorithm:
1. `std::env::current_dir()` → `cwd`.
2. `core::find_manifest(&cwd)` — if `Ok`: print "Already initialised." and return error.
3. Check parent dirs: if any ancestor has `envy.toml`, warn and return error (no nested projects).
4. Generate `project_id = uuid::Uuid::new_v4().to_string()`.
5. `crypto::get_or_create_master_key(&project_id)` → `master_key`.
6. `Vault::open(vault_path(), master_key.as_ref())` — opens/creates the vault file.
7. `vault.create_project(&ProjectId(project_id.clone()))` — registers the project in the DB.
8. `core::create_manifest(&cwd, &project_id)` — writes `envy.toml`.
9. Print success message.

#### `cmd_set` design

`assignment` parsing: split on first `=` only.

```rust
let (key, value) = assignment.split_once('=')
    .ok_or(CliError::InvalidAssignment(assignment.to_owned()))?;
```

Then: `core::set_secret(vault, master_key, project_id, env, key, value)?`

Print: `"✓ Set {key} in {env}."`

#### `cmd_get` design

```rust
let value = core::get_secret(vault, master_key, project_id, env, key)?;
println!("{}", *value);   // Zeroizing<String> deref
```

Only the value is printed to stdout — no labels, no newlines beyond the automatic one from `println!`.

#### `cmd_list` design

```rust
let keys = core::list_secret_keys(vault, project_id, env)?;
for k in &keys {
    println!("{k}");
}
if keys.is_empty() {
    eprintln!("(no secrets in {env})");
}
```

Values are never accessed or printed.

#### `cmd_rm` design

```rust
core::delete_secret(vault, project_id, env, key)?;
println!("✓ Deleted {key} from {env}.");
```

#### `cmd_run` design

This is the most critical handler for correctness (exit code proxying).

```rust
let secrets = core::get_env_secrets(vault, master_key, project_id, env)?;

let (bin, args) = command.split_first().expect("clap ensures non-empty");

let status = std::process::Command::new(bin)
    .args(args)
    .envs(secrets.iter().map(|(k, v)| (k, v.as_str())))
    .status();

match status {
    Ok(s) => s.code().unwrap_or(1),   // `None` on Unix signal termination
    Err(e) => {
        eprintln!("error: failed to execute `{}`: {}", bin, e);
        127   // conventional "command not found" exit code
    }
}
```

> Signal termination (`status.code()` returns `None` on Unix when a child is killed by signal): return exit code 1 as a safe default. This is acceptable for MVP scope (full signal forwarding is Phase 3).

#### `cmd_migrate` design

```rust
let content = std::fs::read_to_string(file)
    .map_err(|e| CliError::FileNotFound(file.display().to_string(), e.to_string()))?;

let mut imported = 0usize;
let mut warnings = 0usize;

for (line_no, line) in content.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        continue;
    }
    match trimmed.split_once('=') {
        Some((key, value)) => {
            core::set_secret(vault, master_key, project_id, env, key.trim(), value)?;
            imported += 1;
        }
        None => {
            eprintln!("warning: line {}: skipping malformed entry: {:?}", line_no + 1, trimmed);
            warnings += 1;
        }
    }
}

println!("✓ Imported {} secret(s) into {env}{}.",
    imported,
    if warnings > 0 { format!(" ({warnings} warning(s))") } else { String::new() }
);
```

---

### 4.3 `src/cli/error.rs` — Error Formatting

Two enums to cover all error surfaces:

```rust
/// CLI-specific errors that do not originate from Core.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("invalid assignment \"{0}\": expected KEY=VALUE format")]
    InvalidAssignment(String),
    #[error("file not found: {0}: {1}")]
    FileNotFound(String, String),
    #[error("already initialised: envy.toml exists in this directory")]
    AlreadyInitialised,
    #[error("parent project detected: {0} already contains envy.toml")]
    ParentProjectExists(String),
    #[error("project not found in vault — was the vault file moved?")]
    ProjectNotInVault,
    #[error("could not open vault: {0}")]
    VaultOpen(String),
    #[error("no command specified after `--`")]
    NoCommand,
}

/// Formats any error (CoreError or CliError) for terminal display.
/// Output is always prefixed with "error: " for visual consistency.
pub fn format_core_error(e: &CoreError) -> String { ... }
pub fn format_cli_error(e: &CliError) -> String { ... }
```

**Exit code mapping**:

| Scenario | Exit code |
|---|---|
| Success | 0 |
| Not an Envy project (`ManifestNotFound`) | 1 |
| Secret not found (`DbError::NotFound`) | 1 |
| Invalid key / assignment | 2 |
| Already initialised / parent project | 3 |
| Vault open / crypto failure | 4 |
| Child command not found (`cmd_run`) | 127 |
| Child command exited with N | N (proxied) |

---

## 5. Key Design Decisions

### 5.1 Vault Lifecycle Ownership

The CLI owns the `Vault` from `Vault::open` to the end of `run()`. The vault is opened once per command invocation. No command shares a vault across invocations. The `master_key: Zeroizing<[u8; 32]>` is dropped at the end of `run()`.

### 5.2 Environment Name Resolution

The `-e / --env` flag is `Option<String>`. The CLI passes an empty string when the flag is absent. Core's `normalize_env()` (already implemented) maps `""` → `"development"`. This keeps the CLI thin — no defaulting logic in the CLI layer.

```rust
let env = args.env.as_deref().unwrap_or("");
```

### 5.3 `set` First-`=` Split

`clap` receives the entire `KEY=VALUE` token as a single positional argument. Splitting on the first `=` via `.split_once('=')` is sufficient and handles values like `abc=def=ghi` correctly.

### 5.4 `run` exit code proxying

`std::process::Command::status()` returns a `std::process::ExitStatus`. On all platforms, `.code()` returns `Option<i32>`. We proxy the code exactly; on Unix signal termination (`None`) we return 1.

### 5.5 `migrate` atomicity

`migrate` is NOT atomic: if the process is killed mid-way, partial secrets are left in the vault. This is acceptable for MVP — the user can run `migrate` again (upsert semantics in `set_secret` mean duplicates are overwritten). A future phase may add transaction support.

### 5.6 No `dirs` crate if avoidable

If `dirs` is not already in `Cargo.toml`, use `std::env::var("HOME")` as a fallback on Unix / `USERPROFILE` on Windows. Add `dirs = "5"` only if the cross-platform logic would otherwise be too complex.

---

## 6. Tests

### Unit tests (in `src/cli/commands.rs` or `src/cli/error.rs`)

These tests operate on pure functions — no file I/O, no vault access:

| Test | What it verifies |
|---|---|
| `parse_assignment_basic` | `"KEY=VALUE"` → `("KEY", "VALUE")` |
| `parse_assignment_value_contains_equals` | `"TOKEN=abc=def"` → `("TOKEN", "abc=def")` |
| `parse_assignment_no_equals` | `"NOVALUE"` → `CliError::InvalidAssignment` |
| `parse_assignment_empty_key` | `"=VALUE"` → key is `""` → `CoreError::InvalidSecretKey` caught by core |
| `migrate_skips_comments_and_blanks` | Lines starting with `#` and empty lines are skipped |
| `migrate_warns_on_malformed` | Line without `=` triggers warning, does not abort |
| `format_manifest_not_found` | `CoreError::ManifestNotFound` formats without stack trace |
| `exit_code_not_found` | `ManifestNotFound` → exit code 1 |
| `exit_code_invalid_key` | `InvalidSecretKey` → exit code 2 |

### Integration test (in `tests/cli_integration.rs`)

One end-to-end test per user story using `assert_cmd` or `std::process::Command`:

| Test | What it verifies |
|---|---|
| `cli_init_creates_manifest` | `envy init` creates `envy.toml` in cwd |
| `cli_set_and_get_round_trip` | `envy set K=V` then `envy get K` prints `V\n` |
| `cli_list_never_shows_values` | `envy list` output matches `^KEY$` (no values) |
| `cli_rm_then_get_fails` | `envy rm K` then `envy get K` exits non-zero |
| `cli_run_injects_secrets` | `envy run -- printenv KEY` prints the secret value |
| `cli_run_proxies_exit_code` | `envy run -- sh -c 'exit 42'` exits with 42 |
| `cli_migrate_imports_env_file` | `envy migrate fixture.env` imports all valid pairs |

---

## 7. Memory Safety

| Surface | Risk | Mitigation |
|---|---|---|
| `master_key` in CLI | Key lives on stack as `Zeroizing<[u8; 32]>` | Zeroized on drop at end of `run()` |
| `get_secret` return | `Zeroizing<String>` printed then dropped | Zeroed immediately after `println!` |
| `cmd_run` secrets | `HashMap<String, Zeroizing<String>>` | Each value zeroed when map is dropped after `Command::status()` returns |
| `migrate` values | Plaintext values from `.env` file | Values passed directly to `set_secret` as `&str`; never heap-allocated separately |

---

## 8. File Modification Summary

| File | Action |
|---|---|
| `src/main.rs` | Replace stub with `process::exit(envy::cli::run())` |
| `src/cli/mod.rs` | Replace stub with Cli struct + Subcommand enum + `pub fn run()` |
| `src/cli/commands.rs` | Create — 7 handler functions |
| `src/cli/error.rs` | Create — `CliError`, `format_core_error`, `format_cli_error`, exit codes |
| `Cargo.toml` | Add `dirs = "5"` if needed for home-dir resolution |
| `tests/cli_integration.rs` | Create — 7 integration tests |
