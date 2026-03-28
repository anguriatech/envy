# Envy — Developer Guide

A living reference for contributors working on this codebase. Updated as the project grows.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Prerequisites](#2-prerequisites)
3. [Project Structure](#3-project-structure)
4. [The 4-Layer Architecture](#4-the-4-layer-architecture)
5. [Key Rust Patterns Used Here](#5-key-rust-patterns-used-here)
6. [Working with the Database Layer](#6-working-with-the-database-layer)
7. [Sync Artifact Architecture](#7-sync-artifact-architecture)
8. [CLI Design Decisions](#8-cli-design-decisions)
9. [Writing Tests](#9-writing-tests)
10. [Daily Development Commands](#10-daily-development-commands)
11. [Common Mistakes to Avoid](#11-common-mistakes-to-avoid)

---

## 1. Project Overview

Envy is a CLI tool that replaces plaintext `.env` files with an encrypted local vault
(`~/.envy/vault.db`). Secrets are stored in SQLite encrypted at the file level by
SQLCipher (AES-256), and each individual secret value is additionally encrypted with
AES-256-GCM before being written to the database (defense in depth).

The master encryption key never touches disk — it lives exclusively in the OS Credential
Manager (macOS Keychain, Windows Credential Manager, Linux Secret Service).

For team collaboration, Envy provides a GitOps sync layer: `envy encrypt` seals the
vault into a single `envy.enc` artifact using Argon2id key derivation and AES-256-GCM
per-environment encryption, making it safe to commit to any repository.

---

## 2. Prerequisites

### Ubuntu / Debian

```bash
# Required before cargo build — provides the OS keyring backend for the `keyring` crate
sudo apt-get install -y libsecret-1-dev pkg-config

# Rust stable toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Useful cargo tools
cargo install cargo-audit   # security vulnerability scanner
```

### macOS

`libsecret` is not needed — the `keyring` crate uses the native Keychain API. Xcode
Command Line Tools must be installed (`xcode-select --install`) for the C compiler
required by the SQLCipher build.

### Verify your setup

```bash
rustc --version       # should show stable, e.g. rustc 1.85.0
cargo --version
pkg-config --version  # Ubuntu only
```

---

## 3. Project Structure

```
envy/
├── src/
│   ├── main.rs           # Binary entry point — wires up clap and calls cli::run()
│   ├── cli/
│   │   ├── mod.rs        # Commands enum, run() dispatch, artifact_path() helper
│   │   ├── commands.rs   # cmd_* handlers (pub(super)) — one function per subcommand
│   │   └── error.rs      # CliError enum, exit-code mappers, formatting helpers
│   ├── core/
│   │   ├── mod.rs        # Re-exports — public face of the business logic layer
│   │   ├── ops.rs        # set_secret, get_secret, list_secret_keys, delete_secret, get_env_secrets
│   │   ├── manifest.rs   # find_manifest, create_manifest, Manifest struct
│   │   ├── sync.rs       # seal_artifact, unseal_artifact, write_artifact, read_artifact
│   │   ├── diff.rs       # compute_diff — pure diff logic (ChangeType, DiffEntry, DiffReport)
│   │   ├── status.rs     # derive_sync_status, get_status_report
│   │   └── error.rs      # CoreError enum
│   ├── crypto/
│   │   ├── mod.rs        # Re-exports — public face of the cryptography layer
│   │   ├── aead.rs       # AES-256-GCM encrypt/decrypt, EncryptedSecret
│   │   ├── artifact.rs   # Argon2id KDF, seal_envelope, unseal_envelope, SyncArtifact types
│   │   ├── keyring.rs    # get_or_create_master_key (OS credential store)
│   │   └── error.rs      # CryptoError enum
│   └── db/
│       ├── mod.rs        # Vault struct, open/close, newtype IDs
│       ├── schema.rs     # CREATE TABLE DDL + migration runner
│       ├── projects.rs   # Project CRUD
│       ├── environments.rs  # Environment CRUD
│       ├── secrets.rs    # Secret upsert/get/list/delete
│       └── error.rs      # DbError enum
│
├── tests/
│   ├── db.rs                # Integration tests for the database layer
│   ├── sync_artifact.rs     # E2E integration tests for the envy.enc pipeline
│   └── cli_integration.rs   # CLI integration tests (require OS keyring; ignored in CI)
│
├── docs/
│   └── developer-guide.md   # This file
├── specs/                   # Feature specs, plans, contracts, and task lists
└── Cargo.toml
```

---

## 4. The 4-Layer Architecture

This is the most important rule in the codebase. Dependencies flow in **one direction only**:

```
┌─────────────┐
│   cli/      │  ← user types a command
└──────┬──────┘
       │ calls
┌──────▼──────┐
│   core/     │  ← orchestrates the operation, enforces business rules
└──────┬──────┘
       │ calls
┌──────▼──────┬──────────────┐
│   crypto/   │    db/       │  ← independent; neither knows about the other
└─────────────┴──────────────┘
```

**What this means in practice:**

| Layer | Can import | CANNOT import |
|-------|-----------|---------------|
| `cli` | `core` | `db`, `crypto` (with two exceptions below) |
| `core` | `db`, `crypto` | `cli` |
| `crypto` | (nothing from this project) | `cli`, `core`, `db` |
| `db` | (nothing from this project) | `cli`, `core`, `crypto` |

**The two CLI exceptions**: `cli` is permitted to call `crate::db::Vault::open` and
`crate::crypto::get_or_create_master_key` directly, because these are infrastructure
bootstrap operations that must happen before `core` functions can be called. Everything
else in `cli` routes through `core`.

**Why it matters**: Every layer can be tested in isolation. Security audits of the
database layer don't require understanding the CLI. The crypto layer has no idea what
a "project" or "environment" is — it just encrypts and decrypts bytes.

If you find yourself wanting to import `db` from `cli`, stop — add a function to `core`
instead and call that.

---

## 5. Key Rust Patterns Used Here

### 5.1 Error handling with `?` and `thiserror`

Rust doesn't have exceptions. Errors are values, returned as `Result<T, E>`.

```rust
// The ? operator: if the function returns Err, propagate it to the caller immediately.
// This replaces try/catch from other languages.
fn get_project(id: &ProjectId) -> Result<Project, DbError> {
    let row = conn.query_row("SELECT ...", params![id.as_str()], |r| {
        // map the row columns into a Project struct
        Ok(Project { id: ProjectId(r.get(0)?), name: r.get(1)? })
    })?; // <-- the ? here propagates any rusqlite error as DbError
    Ok(row)
}
```

The `thiserror` crate lets us define a clean error enum with human-readable messages:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("record not found")]
    NotFound,

    #[error("record already exists")]
    AlreadyExists,

    #[error("internal database error: {0}")]
    Internal(String),
}
```

### 5.2 Newtype wrappers for IDs

A `ProjectId`, `EnvId`, and `SecretId` are all `String` under the hood, but the type
system prevents you from accidentally passing an `EnvId` where a `ProjectId` is expected.

```rust
pub struct ProjectId(pub String);

impl ProjectId {
    pub fn as_str(&self) -> &str { &self.0 }
}
```

### 5.3 The `unwrap()` rule

`.unwrap()` and `.expect()` cause a **panic** (crash) if the value is `Err` or `None`.
They are **prohibited** in production code in this project.

```rust
// ❌ WRONG — crashes the process if anything goes wrong
let conn = Connection::open(path).unwrap();

// ✅ CORRECT — propagates the error to the caller
let conn = Connection::open(path).map_err(|e| DbError::Internal(e.to_string()))?;
```

The only exception: `.expect("reason")` is allowed in **test code** when the reason
makes a panic logically impossible. Document the reason inline:

```rust
// In tests only:
let dir = tempfile::tempdir().expect("tempdir always succeeds in a writable OS");
```

### 5.4 Splitting a struct's methods across multiple files

Rust allows multiple `impl` blocks for the same type, even across different files. Envy
uses this to keep the `Vault` struct clean:

```
src/db/mod.rs          ← defines the Vault struct, open/close
src/db/projects.rs     ← impl Vault { create_project, get_project, ... }
src/db/environments.rs ← impl Vault { create_environment, get_environment, ... }
src/db/secrets.rs      ← impl Vault { upsert_secret, get_secret, ... }
```

Each file adds its own `impl Vault { ... }` block. The compiler merges them all.
Private fields like `self.conn` are accessible because these files are child modules
of `db/`, and private fields are visible to all child modules.

### 5.5 Passing byte slices vs owned `Vec<u8>`

The database layer accepts `&[u8]` (a borrowed slice) for encrypted blobs and returns
`Vec<u8>` (owned). This avoids unnecessary copies:

```rust
let ciphertext: Vec<u8> = crypto::encrypt(&master_key, value)?;
vault.upsert_secret(&env_id, "MY_KEY", &ciphertext, &nonce)?;
//                                      ^^^^^^^^^^^ borrowed slice, not moved
```

### 5.6 `Zeroizing<T>` for sensitive values

Secret values and passphrases are wrapped in `zeroize::Zeroizing<T>`, which zeroes the
backing memory when the value is dropped. This is enforced by Constitution Principle I.

```rust
// Secret string from env var — immediately wrapped.
let passphrase = Zeroizing::new(std::env::var("ENVY_PASSPHRASE")?);

// Decrypted value from vault — memory zeroed after use.
let value: Zeroizing<String> = core::get_secret(&vault, &key, &project_id, env, key)?;
```

Never store secret values in a plain `String` — they are not zeroed on drop.

---

## 6. Working with the Database Layer

### 6.1 Opening a vault

```rust
let vault = Vault::open(Path::new("/tmp/test.db"), &master_key)?;
// Vault::open internally runs:
//   PRAGMA key = '...';
//   PRAGMA foreign_keys = ON;
//   PRAGMA journal_mode = WAL;
//   -- then runs schema migrations
```

The master key must be exactly 32 bytes. In production it comes from the OS keyring
via the `keyring` crate. In tests, use a dummy key:

```rust
let master_key = [0u8; 32]; // dummy key for tests — strength doesn't matter here
```

### 6.2 The pragma ordering requirement (critical)

SQLCipher requires `PRAGMA key` to be the **first** statement executed on a new
connection. If you execute any other SQL before setting the key, SQLCipher will return
a "file is not a database" error. This is enforced in `Vault::open` — never bypass it.

### 6.3 Wrong key errors surface at `PRAGMA journal_mode`, not at `PRAGMA key`

Setting `PRAGMA key` **never fails**, even with a completely wrong key. SQLCipher only
needs to decrypt the database file header when it first performs a real read or write
operation. In `Vault::open`, that first real operation is `PRAGMA journal_mode = WAL`
— so that is where `DbError::EncryptionError` is actually returned when the key is wrong.

```
PRAGMA key = "x'ff...'"      ← always Ok(())
PRAGMA foreign_keys = ON     ← Ok(()) — session flag, no file access
PRAGMA journal_mode = WAL    ← Err(EncryptionError) ← HERE for a wrong key
run_migrations               ← would also catch it, but never reached
```

### 6.4 Foreign keys are OFF by default in SQLite

Foreign key enforcement is **disabled** by default and must be enabled per connection
with `PRAGMA foreign_keys = ON`. `Vault::open` sets this automatically. If you ever
open a raw `rusqlite::Connection` in a test, set it manually:

```rust
conn.execute_batch("PRAGMA foreign_keys = ON;")?;
```

### 6.5 Upsert pattern for secrets

SQLite's `INSERT OR REPLACE` is used to implement the "set overwrites existing value"
behavior for secrets. It works against the `UNIQUE(environment_id, key)` constraint:

```rust
conn.execute(
    "INSERT OR REPLACE INTO secrets
     (id, environment_id, key, value_encrypted, value_nonce, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'), strftime('%s','now'))",
    params![id, env_id.as_str(), key, value_encrypted, value_nonce],
)?;
```

---

## 7. Sync Artifact Architecture

This section covers the technical design behind `envy.enc` — the encrypted GitOps
artifact produced by `envy encrypt` and consumed by `envy decrypt`.

**File placement:** `envy.enc` is always written to the same directory as `envy.toml`
(the project root), not to the directory from which you invoke the command. This is
resolved by `artifact_path()` in `src/cli/mod.rs`, which takes the directory returned
by `find_manifest` and joins `"envy.enc"` directly onto it — no `.parent()` traversal.
The result is that `envy.enc` lives alongside `envy.toml` and `.git/`, making it a
natural GitOps artifact: one `git add envy.enc envy.toml` is all that's needed.

### 7.1 The two keys: master key vs. passphrase

Envy uses two completely distinct cryptographic secrets, each with a different purpose
and lifetime:

| | Vault master key | Artifact passphrase |
|---|---|---|
| **Purpose** | Encrypts secrets at rest in `~/.envy/vault.db` | Encrypts the `envy.enc` artifact for sharing |
| **Stored in** | OS credential manager (Keychain / Secret Service) | Not stored — entered by user or set as `ENVY_PASSPHRASE` |
| **Scope** | Per machine, per user | Per team, per project |
| **Env var** | (never set via env var — OS-managed only) | `ENVY_PASSPHRASE` |
| **Rotation** | Rotated by deleting the keyring entry | Rotated by re-sealing with a new passphrase and committing |
| **Format** | 32 random bytes (AES-256 key) | Arbitrary human-readable string |

These two secrets must never be confused. A teammate who knows the `ENVY_PASSPHRASE`
cannot access your local vault. Your local vault master key cannot decrypt an `envy.enc`
sealed by a teammate with a different passphrase.

### 7.2 The `envy.enc` JSON structure

`envy.enc` is a JSON file with a top-level `version` field and an `environments` map
where each key is an environment name and each value is a self-describing encrypted
envelope:

```json
{
  "version": 1,
  "environments": {
    "development": {
      "ciphertext": "<base64>",
      "nonce": "<base64>",
      "kdf": {
        "algorithm": "argon2id",
        "memory_kib": 65536,
        "time_cost": 3,
        "parallelism": 4,
        "salt": "<base64>"
      }
    },
    "production": {
      "ciphertext": "<base64>",
      ...
    }
  }
}
```

Key design decisions:

- **`BTreeMap` for environment ordering** — Both the top-level `environments` map and
  the `KdfParams` fields use `BTreeMap` (or alphabetically-ordered serialization). This
  produces deterministic, alphabetically-sorted JSON keys on every platform, so `git diff`
  shows only the environments that actually changed — not random reorderings.

- **Self-describing envelopes** — Every envelope embeds all the parameters needed to
  re-derive the decryption key (Argon2 memory, time, parallelism, salt). You can decrypt
  any individual envelope without any external metadata or a version lookup.

- **Per-environment independence** — Each environment is encrypted independently. This
  is what makes Progressive Disclosure possible: `development` and `production` can use
  different passphrases, and the format supports this natively.

- **Zero plaintext leakage** — Secret key names and values are serialized to JSON
  *inside* the payload, then the entire payload is encrypted. The outer `envy.enc` file
  contains no key names, no values, and no project identifiers — it is safe for public
  repositories.

### 7.3 Cryptography stack

```
Passphrase (user input)
    │
    ▼  Argon2id (64 MiB, 3 iterations, parallelism 4)
256-bit derived key
    │
    ▼  AES-256-GCM (random 96-bit nonce)
Ciphertext + authentication tag (16 bytes appended)
    │
    ▼  Base64 (constant-time encoding via base64ct)
Stored in envy.enc envelope
```

**Argon2id** (from the `argon2` crate) is the 2015 Password Hashing Competition winner.
It is memory-hard and side-channel resistant, making brute-force attacks against the
passphrase computationally expensive even with GPUs. The embedded KDF parameters mean
the cost can be increased in a future schema version without breaking existing artifacts.

**AES-256-GCM** provides authenticated encryption — any modification to the ciphertext
or associated data is detected before any plaintext is returned. This is the mechanism
behind Progressive Disclosure: when the wrong passphrase is used, the wrong key is
derived, the GCM authentication tag fails, and the envelope is silently skipped rather
than returning garbage plaintext.

The nonce (96-bit random value) is generated fresh for every seal operation, so
re-sealing the same secrets produces different ciphertext each time — preventing
ciphertext comparison attacks.

### 7.4 Progressive Disclosure implementation

The `unseal_artifact` function in `src/core/sync.rs` iterates every environment
independently:

```rust
for (env_name, envelope) in &artifact.environments {
    match unseal_envelope(passphrase, env_name, envelope) {
        Ok(payload) => {
            imported.insert(env_name.clone(), payload.secrets);
        }
        Err(_) => {
            // ALL errors → graceful skip; never abort.
            // AES-GCM authentication failure is indistinguishable from
            // a wrong passphrase — both map to the same graceful skip.
            skipped.push(env_name.clone());
        }
    }
}
```

The CLI layer (`cmd_decrypt`) then checks whether `imported.is_empty()`. If so, it
returns `CliError::NothingImported` (exit 1). If at least one environment was imported,
it exits 0 regardless of how many were skipped — this is the correct UX for a developer
with partial access.

### 7.5 Layer responsibilities

```
src/crypto/artifact.rs  ← pure crypto: Argon2id KDF, AES-256-GCM, Base64, types
src/core/sync.rs        ← orchestration: vault I/O, JSON serialization, file R/W
src/cli/commands.rs     ← UX: passphrase resolution, coloured output, exit codes
```

`crypto/artifact.rs` knows nothing about files, vaults, or the CLI. `core/sync.rs`
knows nothing about terminal colours or exit codes. This separation means each layer
can be tested and audited in isolation.

---

## 8. CLI Design Decisions

### 8.1 Passphrase input: `dialoguer`

The `dialoguer` crate (v0.11) was chosen for passphrase input over `rpassword` and
`inquire` for three reasons:

1. **Hidden input** — `Password::with_theme()` suppresses character echo automatically
   across all supported platforms.
2. **Built-in confirmation** — `with_confirmation("Confirm passphrase", "do not match")`
   handles the double-entry flow for `envy encrypt` without any manual state management.
3. **TTY awareness** — It fails cleanly (returning an error, not hanging) when stdin is
   not a terminal, which is the correct behaviour for CI/CD detection.

The `console` crate (v0.15, brought transitively by `dialoguer`) provides TTY-aware
colour output. When stdout is not a TTY (piped or redirected), ANSI colour codes are
suppressed automatically — no `--no-color` flag needed.

Access pattern: `dialoguer::console::style("✓").green()` — use the re-export, not a
standalone `console` dependency.

### 8.2 Passphrase resolution: `resolve_passphrase`

Both `cmd_encrypt` and `cmd_decrypt` share a single private helper:

```rust
fn resolve_passphrase(prompt: &str, confirm: bool) -> Result<Zeroizing<String>, CliError> {
    // 1. Check ENVY_PASSPHRASE env var (headless CI/CD mode).
    if let Ok(val) = std::env::var("ENVY_PASSPHRASE") {
        if !val.trim().is_empty() {
            return Ok(Zeroizing::new(val));
        }
        // Whitespace-only → treated as unset; fall through to prompt.
    }

    // 2. Interactive terminal prompt (hidden input, optional confirmation).
    let raw = if confirm {
        dialoguer::Password::with_theme(...)
            .with_confirmation("Confirm passphrase", "Passphrases do not match.")
            .interact()?
    } else {
        dialoguer::Password::with_theme(...).interact()?
    };

    // 3. Validate non-empty.
    if raw.trim().is_empty() {
        return Err(CliError::PassphraseInput("passphrase must not be empty".into()));
    }
    Ok(Zeroizing::new(raw))
}
```

- `cmd_encrypt` calls `resolve_passphrase("Enter passphrase", true)` — confirmation required.
- `cmd_decrypt` calls `resolve_passphrase("Enter passphrase", false)` — single entry.
- The returned value is `Zeroizing<String>`, so the passphrase is zeroed from memory
  as soon as the function that received it returns.

### 8.3 `ENVY_PASSPHRASE` in tests (Rust edition 2024)

Rust edition 2024 made `std::env::set_var` and `std::env::remove_var` `unsafe`,
because they are not thread-safe. Tests that set `ENVY_PASSPHRASE` must:

1. Acquire a shared mutex before touching the environment.
2. Wrap `set_var` / `remove_var` calls in `unsafe {}` blocks with a comment explaining
   the safety invariant.

```rust
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn some_test_that_uses_env_var() {
    let _guard = ENV_LOCK.lock().unwrap();

    // SAFETY: single-threaded access serialised by ENV_LOCK above.
    unsafe { std::env::set_var("ENVY_PASSPHRASE", "test-pass") };
    // ... test body ...
    unsafe { std::env::remove_var("ENVY_PASSPHRASE") };
    // _guard drops here, releasing the lock.
}
```

### 8.4 Progressive Disclosure output colours

```rust
// Imported (green checkmark):
println!("  {}  {} ({} secret(s) upserted)",
    dialoguer::console::style("✓").green(),
    env_name, count);

// Skipped (yellow warning, dim):
println!("  {}  {} skipped — different passphrase or key",
    dialoguer::console::style("⚠").yellow().dim(),
    env_name);
```

The `⚠` skipped lines are written to **stdout** (not stderr) because they are
informational — they describe a successful partial operation, not an error. The only
message written to stderr is `format_cli_error` output, which happens in `run()` when
a command returns `Err(...)`.

### 8.5 `envy diff` — transient data model and `Result<bool, CliError>`

`envy diff` is the only `cmd_*` handler that returns `Result<bool, CliError>` instead of
`Result<(), CliError>`. The `bool` represents "differences found" — not an error — and maps
to exit code 1 (following the `diff(1)` convention). The dispatch in `run()` maps:

```rust
Commands::Diff { env, reveal } => {
    match commands::cmd_diff(...) {
        Ok(has_diff) => if has_diff { 1 } else { 0 },
        Err(e) => { eprintln!(...); cli_exit_code(&e) }
    }
}
```

**Architecture**: The diff uses a fully transient data model — no new tables, no schema
migration. `compute_diff()` in `src/core/diff.rs` is a pure function (no I/O) that
accepts two `BTreeMap<String, Zeroizing<String>>` inputs and returns a `DiffReport`. The
CLI layer is responsible for fetching both sides (vault via `core::get_env_secrets`,
artifact via `core::unseal_env`) and rendering the result.

**Passphrase disambiguation**: `unseal_env` returns `Ok(None)` for both "environment not
in artifact" and "wrong passphrase". To distinguish these cases, `cmd_diff` checks
`artifact.environments.contains_key(env_name)` *before* calling `unseal_env`. If the key
is absent, the passphrase prompt is skipped entirely. If present and `unseal_env` returns
`None`, it is an authentication failure.

**Color output**: Unlike `cmd_status` (which uses `comfy-table`), `cmd_diff` uses inline
ANSI escape codes (`\x1b[32m` green, `\x1b[31m` red, `\x1b[33m` yellow) with `NO_COLOR`
and `IsTerminal` detection. No new crate was added.

---

## 9. Writing Tests

### 9.1 Always use `tempfile` for vault isolation

Never use `~/.envy/vault.db` in tests. Each test creates its own temporary file that
is cleaned up automatically when the test ends:

```rust
use tempfile::TempDir;

fn open_test_vault(tmp: &TempDir) -> (Vault, ProjectId) {
    let path = tmp.path().join("vault.db");
    let vault = Vault::open(&path, &[0xABu8; 32]).expect("vault opens");
    let pid = vault.create_project("test").expect("project created");
    (vault, pid)
}

#[test]
fn test_something() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (vault, pid) = open_test_vault(&tmp);
    // ...
}
```

### 9.2 TDD discipline for new handlers

New CLI handlers (`cmd_*`) follow strict TDD:

1. Write tests that compile but fail (stub the function with `todo!()`).
2. Run `cargo test --no-run` — compilation must succeed.
3. Implement the function.
4. Run `cargo test -- <module>` — all tests must pass.

This catches import errors and type mismatches before any implementation work begins.

### 9.3 Integration tests and the OS keyring

Tests in `tests/cli_integration.rs` invoke `envy` as a subprocess and require a live
OS keyring daemon (Linux: Secret Service / libsecret, macOS: Keychain). They are
annotated with `#[ignore]` so they are skipped in CI environments without a keyring.

```bash
# Run all unit tests (default — no keyring needed):
cargo test

# Run integration tests (requires keyring daemon):
cargo test -- --ignored
```

### 9.4 Asserting errors (not panics)

Use pattern matching to assert that an operation returns a specific error variant:

```rust
let result = vault.get_project(&ProjectId("nonexistent-id".to_string()));
assert!(matches!(result, Err(DbError::NotFound)));
```

---

## 10. Daily Development Commands

```bash
# Build the project (first build compiles SQLCipher — takes ~2 minutes)
cargo build

# Run all tests (integration tests skipped in CI — no keyring needed)
cargo test

# Run tests with println! output visible
cargo test -- --nocapture

# Run a specific test module
cargo test cli::commands
cargo test core::sync
cargo test db::

# Run integration tests (requires live keyring daemon)
cargo test -- --ignored

# Check for warnings (all warnings are errors in this project)
cargo clippy -- -D warnings

# Security audit of dependencies
cargo audit

# Format code
cargo fmt

# Check formatting without changing files (useful in CI)
cargo fmt --check
```

---

## 11. Common Mistakes to Avoid

| Mistake | What goes wrong | Correct approach |
|---------|----------------|-----------------|
| Using `.unwrap()` in `src/` | Process panics in production on any error | Use `?` to propagate, or `.map_err()` to convert |
| Importing `db` or `crypto` from `cli` (beyond the two exceptions) | Violates 4-layer architecture | Add a function to `core` and call that |
| Opening a raw `Connection` without `PRAGMA foreign_keys = ON` | Cascade deletes and FK constraints silently do nothing | Always use `Vault::open`, never bypass it |
| Executing SQL before `PRAGMA key` | "file is not a database" error from SQLCipher | Always use `Vault::open` which sets the key first |
| Using `~/.envy/vault.db` path in tests | Tests corrupt the real user vault | Use `tempfile::TempDir` in every test |
| Storing the master key or passphrase in a plain `String` | Secrets are not zeroed on drop | Use `Zeroizing<String>` or `Zeroizing<Vec<u8>>`; zero after use |
| Logging `value_encrypted`, `master_key`, or passphrase | Leaks sensitive data to stdout/logs | These MUST NEVER appear in log output or error messages |
| Mapping `PRAGMA journal_mode` errors to `Internal` | Wrong-key errors are silently reclassified | Use `map_err(map_rusqlite_error)` for statements in `Vault::open` |
| Calling `std::env::set_var` in tests without `ENV_LOCK` | Parallel tests corrupt each other's environment | Acquire the shared mutex; wrap in `unsafe {}` with safety comment |
| Confusing `ENVY_PASSPHRASE` with the vault master key | They are completely different secrets with different scopes | See §7.1 — master key is OS-managed; passphrase is team-shared |
| Treating a skipped environment in `unseal_artifact` as an error | Breaks Progressive Disclosure — partial access is valid | Skipped environments go in `UnsealResult.skipped`; only `imported.is_empty()` is an error |
| Writing the `⚠` skipped line to stderr | Confuses tooling that expects errors on stderr | Skipped lines are informational — they belong on stdout |
