# Contract: CLI Public API Surface

**Feature**: 004-cli-interface
**Date**: 2026-03-19
**Stability**: Draft — awaiting approval

This document defines the public API surface of `src/cli/`, the command-line argument structures, error-to-exit-code mapping, and the `run` command lifecycle contract. The only caller of `cli::run()` is `main.rs`.

---

## Entry Point

```rust
// src/cli/mod.rs — called exclusively by main.rs
pub fn run() -> i32
```

`main.rs` must call `std::process::exit(envy::cli::run())`. The return value is always a valid POSIX exit code (0–255). Panics are prohibited in all code paths reachable from `run()`.

---

## Clap Struct Hierarchy

```rust
// Derive-based clap structures — the source of truth for terminal argument parsing.

#[derive(clap::Parser)]
#[command(name = "envy", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    Init,
    Set    { assignment: String, #[arg(short='e', long="env")] env: Option<String> },
    Get    { key: String,        #[arg(short='e', long="env")] env: Option<String> },
    #[command(alias = "ls")]
    List   {                     #[arg(short='e', long="env")] env: Option<String> },
    #[command(alias = "remove")]
    Rm     { key: String,        #[arg(short='e', long="env")] env: Option<String> },
    Run    { #[arg(short='e', long="env")] env: Option<String>,
             #[arg(last=true, required=true)] command: Vec<String> },
    Migrate { file: std::path::PathBuf, #[arg(short='e', long="env")] env: Option<String> },
}
```

### Global `-e / --env` rule

- All commands except `Init` accept `-e, --env <ENV>`.
- When absent, the CLI passes `""` to Core functions. Core normalises `""` → `"development"`.
- The CLI MUST NOT hardcode the default environment name — Core owns that constant.

---

## Handler Function Signatures

All handler functions are `pub(super)` — only callable from `cli::mod`.

```rust
// src/cli/commands.rs

pub(super) fn cmd_init() -> Result<(), CliError>;

pub(super) fn cmd_set(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env:        &str,       // "" when -e flag absent
    assignment: &str,       // full "KEY=VALUE" token from clap
) -> Result<(), CoreError>;

pub(super) fn cmd_get(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env:        &str,
    key:        &str,
) -> Result<(), CoreError>;

pub(super) fn cmd_list(
    vault:      &Vault,
    project_id: &ProjectId,
    env:        &str,
) -> Result<(), CoreError>;

pub(super) fn cmd_rm(
    vault:      &Vault,
    project_id: &ProjectId,
    env:        &str,
    key:        &str,
) -> Result<(), CoreError>;

/// Returns the child exit code directly (not a Result).
pub(super) fn cmd_run(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env:        &str,
    command:    &[String],  // first element is the binary, rest are args
) -> i32;

pub(super) fn cmd_migrate(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env:        &str,
    file:       &Path,
) -> Result<(), CliError>;
```

---

## CliError Enum

```rust
// src/cli/error.rs

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// `set` argument lacked an `=` separator.
    #[error("invalid assignment \"{0}\": expected KEY=VALUE format")]
    InvalidAssignment(String),

    /// `migrate` target file could not be read.
    #[error("cannot read file \"{0}\": {1}")]
    FileNotFound(String, String),

    /// `init` run in a directory that already has envy.toml.
    #[error("already initialised: envy.toml exists in this directory")]
    AlreadyInitialised,

    /// `init` run inside an existing Envy project tree.
    #[error("parent project detected: \"{0}\" already contains envy.toml")]
    ParentProjectExists(String),

    /// Vault file exists but `create_project` reports the project_id is absent.
    #[error("project not found in vault — was the vault file moved?")]
    ProjectNotInVault,

    /// Vault::open failed (wrong key, corrupted file, permissions).
    #[error("could not open vault: {0}")]
    VaultOpen(String),
}
```

---

## Error-to-Exit-Code Mapping

```rust
// src/cli/error.rs

pub fn core_exit_code(e: &CoreError) -> i32 {
    match e {
        CoreError::ManifestNotFound        => 1,
        CoreError::Db(DbError::NotFound)   => 1,
        CoreError::ManifestInvalid(_)      => 1,
        CoreError::ManifestIo(_)           => 1,
        CoreError::InvalidSecretKey(_)     => 2,
        CoreError::Db(_)                   => 4,
        CoreError::Crypto(_)               => 4,
    }
}

pub fn cli_exit_code(e: &CliError) -> i32 {
    match e {
        CliError::InvalidAssignment(_)     => 2,
        CliError::FileNotFound(_, _)       => 1,
        CliError::AlreadyInitialised       => 3,
        CliError::ParentProjectExists(_)   => 3,
        CliError::ProjectNotInVault        => 4,
        CliError::VaultOpen(_)             => 4,
    }
}
```

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Not found (manifest, file, secret) |
| 2    | Invalid input (key name, assignment format) |
| 3    | Initialisation conflict |
| 4    | Vault / crypto failure |
| 127  | Child binary not found (run command) |
| N    | Child process exit code (proxied by run) |

---

## `run` Command Lifecycle Contract

```
1. core::get_env_secrets(vault, master_key, project_id, env)
        → HashMap<String, Zeroizing<String>>
2. std::process::Command::new(command[0])
        .args(&command[1..])
        .envs(secrets.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .status()
3. match status:
     Ok(s)  → s.code().unwrap_or(1)   // None on Unix signal kill → 1
     Err(e) → eprintln!(...); 127
```

**Invariants**:
- Secrets are injected **in addition to** the inherited environment (not a replacement).
- The `--` separator is consumed by clap; it does NOT appear in `command: Vec<String>`.
- `command` is guaranteed non-empty by `#[arg(last = true, required = true)]`.
- Secrets (`Zeroizing<String>` values) are zeroed when the HashMap is dropped after `.status()` returns.

---

## `set` Argument Parsing Contract

```
Input:  assignment: &str  (full terminal token, e.g. "TOKEN=abc=def")
Split:  assignment.split_once('=') → Some((key, value))
            key   = "TOKEN"
            value = "abc=def"
        None → CliError::InvalidAssignment

Key validation is performed by core::set_secret (CoreError::InvalidSecretKey).
The CLI does NOT duplicate key validation — Core is the authority.
```

---

## `migrate` Line Parsing Contract

```
For each line in file:
  1. Trim leading/trailing whitespace.
  2. If empty or starts with '#': skip silently.
  3. split_once('='):
       Some((key, value)) → core::set_secret(key.trim(), value)   // imports
       None               → eprintln!("warning: line N: ..."); continue
4. On any core::set_secret error: return Err immediately (abort migration).
5. After all lines: println!("✓ Imported N secret(s) into {env}.")
```

---

## `init` Lifecycle Contract

```
1. current_dir = std::env::current_dir()
2. core::find_manifest(&current_dir):
     Ok(_)  → return Err(CliError::AlreadyInitialised)
     Err(CoreError::ManifestNotFound) → continue
     Err(other) → return Err(CliError::VaultOpen(other.to_string()))
3. Check ancestors for envy.toml (find_manifest walks upward — if Ok on any
   ancestor above current_dir, return Err(CliError::ParentProjectExists(path)))
4. project_id = uuid::Uuid::new_v4().to_string()
5. master_key = crypto::get_or_create_master_key(&project_id)
6. vault = Vault::open(vault_path(), master_key.as_ref())
7. vault.create_project(&ProjectId(project_id.clone()))
8. core::create_manifest(&current_dir, &project_id)
9. println!("✓ Initialised envy project {project_id}.")
```

> Step 3 requires distinguishing "found in current dir" vs "found in ancestor". The simplest approach: after `find_manifest` returns `ManifestNotFound`, we know no manifest exists anywhere above. If instead it returns `Ok((_, found_dir))` and `found_dir != current_dir`, that is the `ParentProjectExists` case.

---

## stdout / stderr Contract

| Command | stdout | stderr |
|---------|--------|--------|
| `init` | success message | errors |
| `set` | success message | errors |
| `get` | **raw value only** (no label, no extra whitespace) | errors |
| `list` | one key per line, alphabetical; empty message if no keys | errors |
| `rm` | success message | errors |
| `run` | child process stdout (inherited) | child process stderr (inherited) + envy errors |
| `migrate` | summary line | per-line warnings + errors |

**Critical invariant for `get`**: stdout contains exactly `{value}\n` and nothing else. Shell pipelines (`envy get KEY | xargs ...`) depend on this.

**Critical invariant for `list`**: secret values MUST NEVER appear in stdout or stderr output, even in error messages or debug output.
