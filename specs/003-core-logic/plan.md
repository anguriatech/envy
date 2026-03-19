# Implementation Plan: 003-core-logic

**Feature**: Core Logic
**Branch**: `003-core-logic`
**Date**: 2026-03-19
**Status**: Awaiting approval

---

## 1. Scope

Implement `src/core/` as the orchestration layer that:

1. **Resolves project context** — walks the directory tree to find `envy.toml` and reads
   the `project_id`.
2. **Manages environment defaulting** — defaults to `"development"`, auto-creates
   environments on first write.
3. **Coordinates secret CRUD** — encrypts before DB write, decrypts after DB read,
   validates key names.
4. **Prepares execution environment** — decrypts all secrets for a given
   project/environment into a zeroing `HashMap` ready for child process injection.

---

## 2. Architecture Position

```
src/
├── cli/        ← (stub) calls Core functions; owns Vault lifecycle
├── core/       ← THIS FEATURE
│   ├── mod.rs          pub re-exports + CoreError
│   ├── error.rs        CoreError enum
│   ├── manifest.rs     envy.toml read/write + context resolution
│   └── ops.rs          secret CRUD + get_env_secrets
├── crypto/     ← (complete) encrypt / decrypt / get_or_create_master_key
└── db/         ← (complete) Vault CRUD
```

**Dependency rule** (Constitution Principle IV):

```
cli → core → crypto   ✓
           → db       ✓
core → cli            ✗  (prohibited)
db  → core            ✗  (prohibited)
crypto → core         ✗  (prohibited)
```

---

## 3. New Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
toml  = "0.8"
serde = { version = "1", features = ["derive"] }
```

> `zeroize = "1"` is already present. No other new crates required.

---

## 4. Module Design

### 4.1 `error.rs` — `CoreError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Db(#[from] DbError),

    /// A cryptographic operation failed.
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),

    /// No envy.toml was found in the current directory or any parent.
    #[error("not an envy project (run `envy init` to initialize)")]
    ManifestNotFound,

    /// envy.toml was found but could not be parsed or is missing required fields.
    #[error("envy.toml is invalid: {0}")]
    ManifestInvalid(String),

    /// An I/O error occurred while reading or writing envy.toml.
    #[error("could not read/write envy.toml: {0}")]
    ManifestIo(String),

    /// The secret key name failed validation (empty, contains `=`, etc.).
    #[error("invalid secret key name \"{0}\": must be non-empty and must not contain `=`")]
    InvalidSecretKey(String),
}
```

**Design notes**:
- `#[from] DbError` and `#[from] CryptoError` enable `?`-based propagation from ops.
- `ManifestNotFound` is separate from `ManifestInvalid` so the CLI can give different
  recovery hints ("run envy init" vs. "check your envy.toml").
- `InvalidSecretKey` is caught before any DB or Crypto call, so it never leaks key names
  into error messages that might be logged.

---

### 4.2 `manifest.rs` — Context Resolution

**TOML schema** (`envy.toml`):

```toml
# Created by `envy init`. Do not delete — this file links the directory to its vault.
project_id = "550e8400-e29b-41d4-a716-446655440000"
```

**Struct** (serde-derived):

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub project_id: String,
}
```

**Public API** (`pub(super)` — re-exported from `mod.rs` as `pub`):

```rust
/// Walks from `start_dir` up to the filesystem root looking for `envy.toml`.
/// Returns the parsed Manifest and the directory it was found in.
pub fn find_manifest(start_dir: &Path) -> Result<(Manifest, PathBuf), CoreError>

/// Creates `envy.toml` in `target_dir` with the given `project_id`.
/// Returns ManifestIo if the file already exists or cannot be written.
pub fn create_manifest(target_dir: &Path, project_id: &str) -> Result<(), CoreError>
```

**`find_manifest` algorithm**:
1. Start at `start_dir` (typically `std::env::current_dir()`).
2. Check if `<dir>/envy.toml` exists.
3. If yes: read and parse via `toml::from_str::<Manifest>` → return `(manifest, dir)`.
4. If parse fails: return `ManifestInvalid(err.to_string())`.
5. If no: move to `dir.parent()`.
6. If no parent (hit root): return `ManifestNotFound`.

---

### 4.3 `ops.rs` — Secret Operations

**Constants**:

```rust
pub const DEFAULT_ENV: &str = "development";
```

**Secret key validation** (private helper):

```rust
fn validate_key(key: &str) -> Result<(), CoreError> {
    if key.is_empty() || key.contains('=') {
        return Err(CoreError::InvalidSecretKey(key.to_owned()));
    }
    Ok(())
}
```

**Environment resolution** (private helper):

```rust
/// Gets the EnvId for the given environment name, creating the environment if
/// it does not yet exist. The name is always lowercased before lookup/creation.
fn resolve_env(
    vault: &Vault,
    project_id: &ProjectId,
    env_name: &str,
) -> Result<EnvId, CoreError>
```

Algorithm:
1. Lowercase `env_name`.
2. `vault.get_environment_by_name(project_id, &lowered)` → if `Ok`, return `id`.
3. If `DbError::NotFound` → `vault.create_environment(project_id, &lowered)` → return new `id`.
4. Other errors → propagate as `CoreError::Db`.

**Public secret operations** (all `pub`):

```rust
/// Encrypts `plaintext` and stores it in the vault under `(project_id, env_name, key)`.
/// Creates the environment if it does not exist. Upsert semantics (last write wins).
pub fn set_secret(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,        // defaults to DEFAULT_ENV if empty
    key:        &str,
    plaintext:  &str,
) -> Result<(), CoreError>
```

Sequence:
1. `validate_key(key)?`
2. Resolve effective env name (if empty → `DEFAULT_ENV`).
3. `resolve_env(vault, project_id, env_name)?` → `env_id`.
4. `crypto::encrypt(master_key, plaintext.as_bytes())?` → `EncryptedSecret { ciphertext, nonce }`.
5. `vault.upsert_secret(&env_id, key, &ciphertext, &nonce)?`.

```rust
/// Fetches and decrypts the secret for `(project_id, env_name, key)`.
/// Returns the plaintext wrapped in Zeroizing (zeroed on drop).
pub fn get_secret(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,
    key:        &str,
) -> Result<Zeroizing<String>, CoreError>
```

Sequence:
1. `validate_key(key)?`
2. Resolve effective env name.
3. `vault.get_environment_by_name(project_id, env_name)?` → `env_id` (no auto-create on read).
4. `vault.get_secret(&env.id, key)?` → `SecretRecord`.
5. `crypto::decrypt(master_key, &record.value_encrypted, &record.value_nonce)?` → `Zeroizing<Vec<u8>>`.
6. Convert bytes to `String` via `String::from_utf8` → wrap in `Zeroizing::new`.

```rust
/// Returns the key names for all secrets in the environment, ordered alphabetically.
/// Does NOT decrypt values — safe to call without the master key.
pub fn list_secret_keys(
    vault:      &Vault,
    project_id: &ProjectId,
    env_name:   &str,
) -> Result<Vec<String>, CoreError>
```

Sequence:
1. Resolve effective env name.
2. `vault.get_environment_by_name(project_id, env_name)?` → `env`.
3. `vault.list_secrets(&env.id)?` → extract `.key` field from each record.

```rust
/// Permanently deletes the secret for `(project_id, env_name, key)`.
/// Returns CoreError::Db(DbError::NotFound) if the key does not exist.
pub fn delete_secret(
    vault:      &Vault,
    project_id: &ProjectId,
    env_name:   &str,
    key:        &str,
) -> Result<(), CoreError>
```

```rust
/// Decrypts ALL secrets for the given environment.
/// Returns a HashMap of key → Zeroizing<String> ready for env-var injection.
/// If any single decryption fails, the entire operation fails (no partial maps).
pub fn get_env_secrets(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,
) -> Result<HashMap<String, Zeroizing<String>>, CoreError>
```

Sequence:
1. Resolve effective env name.
2. `vault.get_environment_by_name(project_id, env_name)?` → `env` (error on missing, no auto-create).
3. `vault.list_secrets(&env.id)?` → `Vec<SecretRecord>`.
4. For each record: `crypto::decrypt(...)` → convert to `Zeroizing<String>`.
5. If any decrypt fails → return error immediately (no partial map).
6. Collect into `HashMap<String, Zeroizing<String>>`.

---

### 4.4 `mod.rs` — Public Re-exports

```rust
mod error;
mod manifest;
mod ops;

pub use error::CoreError;
pub use manifest::{find_manifest, create_manifest, Manifest};
pub use ops::{
    set_secret, get_secret, list_secret_keys, delete_secret, get_env_secrets,
    DEFAULT_ENV,
};
```

---

## 5. Memory Safety Plan

| Value | Held in | Zeroed by |
|-------|---------|-----------|
| Master key (`&[u8; 32]`) | Caller-owned `Zeroizing<[u8; 32]>` | Caller drops it |
| Decrypted bytes | `Zeroizing<Vec<u8>>` from `crypto::decrypt` | `Zeroizing` drop impl |
| Decrypted `String` | `Zeroizing<String>` in return value | `Zeroizing` drop impl |
| `HashMap` values | `Zeroizing<String>` | Each value zeroed when map is dropped |
| Intermediate plaintext bytes | Dropped at end of `get_secret` / `get_env_secrets` scope | Rust drop order |

> `Zeroizing<String>` works because `String: Zeroize` (implemented by the zeroize crate).

---

## 6. Test Plan

Tests live in `src/core/` as `#[cfg(test)]` unit tests. They use `tempfile` (already a
dev-dep) for isolated vault files and `std::env::set_current_dir` / `tempdir` for
manifest resolution tests.

| Test | Location | What it verifies |
|------|----------|-----------------|
| `find_manifest_in_current_dir` | `manifest.rs` | Finds envy.toml in cwd |
| `find_manifest_in_parent_dir` | `manifest.rs` | Walks up to find envy.toml |
| `find_manifest_not_found` | `manifest.rs` | Returns ManifestNotFound at fs root |
| `create_and_read_manifest` | `manifest.rs` | Round-trips project_id through toml |
| `set_and_get_secret_round_trip` | `ops.rs` | Encrypt → store → fetch → decrypt |
| `set_secret_upsert` | `ops.rs` | Second set replaces first value |
| `get_secret_not_found` | `ops.rs` | Db(NotFound) for missing key |
| `list_secret_keys_order` | `ops.rs` | Keys returned alphabetically, no values |
| `delete_secret_removes` | `ops.rs` | Key absent after delete |
| `delete_secret_not_found` | `ops.rs` | Db(NotFound) for missing key |
| `get_env_secrets_all_decrypted` | `ops.rs` | Full HashMap, all values correct |
| `get_env_secrets_empty_env` | `ops.rs` | Empty map, not error |
| `get_env_secrets_partial_fail` | `ops.rs` | One bad ciphertext → whole op fails |
| `default_env_auto_created` | `ops.rs` | set_secret with no env creates "development" |
| `invalid_key_rejected` | `ops.rs` | Empty key and key with = both return InvalidSecretKey |

---

## 7. Constitution Compliance

| Principle | How this feature complies |
|-----------|--------------------------|
| I. Security | Plaintext never logged; all decrypted values in `Zeroizing` wrappers; master key not cloned or stored |
| II. Determinism | Default env is a named constant; directory walk order is deterministic |
| III. Rust Best Practices | `thiserror` + `#[from]` for typed errors; no `.unwrap()` without justification; full unit tests |
| IV. Modularity | `core/` imports only `db/` and `crypto/`; never imports `cli/` |
| V. Language | All identifiers, comments, and docs in English |

---

## 8. Out of Scope

- `envy init` command logic — that belongs to the CLI layer (it calls `create_manifest`
  and `Vault::create_project` from here, but the command parsing is not in Core).
- Project CRUD (create/list/delete projects) — the Core layer exposes secret operations
  only; project management is coordinated by the CLI.
- Key rotation — not in scope for this feature.
- Remote sync, export, or CI/CD headless mode — Phase 2 roadmap items.
