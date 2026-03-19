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
7. [Writing Tests](#7-writing-tests)
8. [Daily Development Commands](#8-daily-development-commands)
9. [Common Mistakes to Avoid](#9-common-mistakes-to-avoid)

---

## 1. Project Overview

Envy is a CLI tool that replaces plaintext `.env` files with an encrypted local vault
(`~/.envy/vault.db`). Secrets are stored in SQLite encrypted at the file level by
SQLCipher (AES-256), and each individual secret value is additionally encrypted with
AES-256-GCM before being written to the database (defense in depth).

The master encryption key never touches disk — it lives exclusively in the OS Credential
Manager (macOS Keychain, Windows Credential Manager, Linux Secret Service).

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
│   ├── main.rs           # Binary entry point — wires up clap and calls Core
│   ├── cli/
│   │   └── mod.rs        # Argument parsing (clap). Output formatting. Nothing else.
│   ├── core/
│   │   └── mod.rs        # Business logic. Calls db/ and crypto/. Never calls cli/.
│   ├── crypto/
│   │   └── mod.rs        # AES-256-GCM encrypt/decrypt, HKDF key derivation
│   └── db/
│       ├── mod.rs        # Vault struct, open/close, newtype IDs
│       ├── schema.rs     # CREATE TABLE DDL + migration runner
│       ├── projects.rs   # Project CRUD
│       ├── environments.rs  # Environment CRUD
│       ├── secrets.rs    # Secret upsert/get/list/delete
│       └── error.rs      # DbError enum
│
├── tests/
│   ├── db.rs             # Integration test entry point (declares submodules)
│   └── db/
│       ├── test_schema.rs
│       ├── test_projects.rs
│       ├── test_environments.rs
│       ├── test_secrets.rs
│       └── test_security.rs
│
├── docs/
│   └── developer-guide.md   # This file
├── specs/                   # Feature specs, plans, and task lists (Specify workflow)
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
| `cli` | `core` | `db`, `crypto` |
| `core` | `db`, `crypto` | `cli` |
| `crypto` | (nothing from this project) | `cli`, `core`, `db` |
| `db` | (nothing from this project) | `cli`, `core`, `crypto` |

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

When a `rusqlite::Error` comes back from the database, we convert it to our `DbError`
using `.map_err()`:

```rust
conn.execute("INSERT INTO ...", params![...])
    .map_err(|e| DbError::Internal(e.to_string()))?;
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

Usage:
```rust
let id = ProjectId("some-uuid".to_string());
vault.get_project(&id)?;   // compiler enforces you pass the right ID type
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

### 5.5 Splitting a struct's methods across multiple files (`impl Vault` in four places)

Rust allows multiple `impl` blocks for the same type, even across different files, as long
as they are all within the same crate. Envy uses this to keep the `Vault` struct clean:

```
src/db/mod.rs          ← defines the Vault struct, open/close, diagnostic helpers
src/db/projects.rs     ← impl Vault { create_project, get_project, ... }
src/db/environments.rs ← impl Vault { create_environment, get_environment, ... }
src/db/secrets.rs      ← impl Vault { upsert_secret, get_secret, ... }
```

Each file adds its own `impl Vault { ... }` block. The compiler merges them all —
`Vault` ends up with methods from every file, and callers see them all on the same type.

**How it works:**

```rust
// src/db/mod.rs — the struct lives here
pub struct Vault {
    conn: rusqlite::Connection,
}

// Vault::open and Vault::close are also implemented here via `impl Vault { ... }`

// src/db/projects.rs — adds more methods to the same Vault type
use super::{Vault, ProjectId};

impl Vault {
    pub fn create_project(&self, name: &str) -> Result<ProjectId, DbError> {
        // `self.conn` is accessible here because projects.rs is a submodule of db/
        ...
    }
}
```

**Why this works (the visibility rule):**
`self.conn` is a private field on `Vault`. Private fields are accessible to code in the
same module AND to all child modules. Since `projects.rs` is declared as `mod projects`
inside `src/db/mod.rs`, it is a child module of `db` — so it can access `conn` directly.

**Why we do it this way:**

| Approach | Problem |
|----------|---------|
| All methods in `mod.rs` | One 800-line file — hard to navigate |
| Separate struct per entity | Callers would need `ProjectVault`, `EnvVault`, etc. — awkward API |
| Methods split across files | ✅ One type, organized source code, compiler merges it all |

**Adding a new entity (e.g., `tags.rs`):**

1. Create `src/db/tags.rs` with `impl Vault { ... }` and the new methods.
2. Add `mod tags;` to `src/db/mod.rs` (that's it — no re-exports needed for the methods).
3. If the new file defines a new struct (like `Tag`), re-export it: `pub use tags::Tag;` in `mod.rs`.

### 5.4 Passing byte slices vs owned `Vec<u8>`

The database layer accepts `&[u8]` (a borrowed slice) for encrypted blobs and returns
`Vec<u8>` (owned). This avoids unnecessary copies:

```rust
// Caller owns the ciphertext Vec, passes a reference to the DB layer
let ciphertext: Vec<u8> = crypto::encrypt(&master_key, value)?;
vault.upsert_secret(&env_id, "MY_KEY", &ciphertext, &nonce)?;
//                                      ^^^^^^^^^^^ borrowed slice, not moved
```

---

## 6. Working with the Database Layer

### 6.1 Opening a vault

```rust
use std::path::Path;

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

### 6.2b Wrong key errors surface at `PRAGMA journal_mode`, not at `PRAGMA key`

This is a subtle SQLCipher behaviour discovered during Phase 2 implementation.

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

**What this means for you:**

- Never assume a successful `PRAGMA key` means the key is correct.
- If you add new statements between `PRAGMA key` and `PRAGMA journal_mode` in
  `Vault::open`, make sure they use `map_err(map_rusqlite_error)` (not
  `.map_err(|e| DbError::Internal(...))`) so that encryption errors are not silently
  reclassified as internal errors.
- The `is_encryption_error` helper in `src/db/error.rs` checks for SQLite error code 26
  (`SQLITE_NOTADB`), which is what SQLCipher returns when decryption fails.

### 6.3 Foreign keys are OFF by default in SQLite

This is a SQLite footgun: foreign key enforcement is **disabled** by default and must
be enabled per connection with `PRAGMA foreign_keys = ON`. `Vault::open` sets this
automatically. If you ever open a raw `rusqlite::Connection` in a test, set it manually:

```rust
conn.execute_batch("PRAGMA foreign_keys = ON;")?;
```

Without this, `ON DELETE CASCADE` and referential integrity checks are silently ignored.

### 6.4 Upsert pattern for secrets

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

Note: `INSERT OR REPLACE` deletes the old row and inserts a new one — meaning the
`id` UUID changes on every update. This is acceptable in Phase 1. Phase 3 audit logs
will require a different strategy (versioned rows).

---

## 7. Writing Tests

### 7.1 Always use `tempfile` for vault isolation

Never use `~/.envy/vault.db` in tests. Each test creates its own temporary file that
is cleaned up automatically when the test ends:

```rust
use tempfile::NamedTempFile;

#[test]
fn test_create_project() {
    let tmp = NamedTempFile::new()
        .expect("tempfile always succeeds in a writable OS");
    let master_key = [0u8; 32];

    let vault = Vault::open(tmp.path(), &master_key)
        .expect("vault opens on a fresh temp file");

    let id = vault.create_project("my-app")
        .expect("create_project on empty vault succeeds");

    assert_eq!(id.as_str().len(), 36); // UUID v4 hyphenated format
}
```

### 7.2 Test file structure

Integration tests live in `tests/db/`. The entry point is `tests/db.rs`:

```rust
// tests/db.rs — declares all submodules
// The #[path] attributes are required because tests/db.rs is itself the crate root.
// Without them, Rust would look for tests/test_schema.rs instead of tests/db/test_schema.rs.
#[path = "db/test_schema.rs"]
mod test_schema;
#[path = "db/test_projects.rs"]
mod test_projects;
#[path = "db/test_environments.rs"]
mod test_environments;
#[path = "db/test_secrets.rs"]
mod test_secrets;
#[path = "db/test_security.rs"]
mod test_security;
```

Each submodule (`tests/db/test_projects.rs` etc.) contains the actual `#[test]`
functions. To run a specific submodule:

```bash
cargo test db::test_projects    # runs only test_projects.rs tests
cargo test db::                 # runs all db integration tests
cargo test                      # runs everything
```

### 7.3 Asserting errors (not panics)

Use pattern matching to assert that an operation returns a specific error variant:

```rust
let result = vault.get_project(&ProjectId("nonexistent-id".to_string()));
assert!(matches!(result, Err(DbError::NotFound)));
```

---

## 8. Daily Development Commands

```bash
# Build the project (first build compiles SQLCipher — takes ~2 minutes)
cargo build

# Run all tests
cargo test

# Run tests with println! output visible
cargo test -- --nocapture

# Run only db integration tests
cargo test db::

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

## 9. Common Mistakes to Avoid

| Mistake | What goes wrong | Correct approach |
|---------|----------------|-----------------|
| Using `.unwrap()` in `src/` | Process panics in production on any error | Use `?` to propagate, or `.map_err()` to convert |
| Importing `db` from `cli` | Violates 4-layer architecture | Add a function to `core` and call that |
| Opening a raw `Connection` without `PRAGMA foreign_keys = ON` | Cascade deletes and FK constraints silently do nothing | Always use `Vault::open`, never bypass it |
| Executing SQL before `PRAGMA key` | "file is not a database" error from SQLCipher | Always use `Vault::open` which sets the key first |
| Using `~/.envy/vault.db` path in tests | Tests corrupt the real user vault | Use `tempfile::NamedTempFile` in every test |
| Logging `value_encrypted` or `master_key` | Leaks sensitive data to stdout/logs | These variables MUST NEVER be logged |
| Storing the master key in a `String` | Strings are not zeroed on drop | Use `Vec<u8>` or a `Zeroize`-implementing type; zero it after use |
| Mapping `PRAGMA journal_mode` errors to `Internal` | Wrong-key errors are silently reclassified, making them undebuggable | Use `map_err(map_rusqlite_error)` for any statement in `Vault::open` that touches the file |
