# Contract: Core Logic Public API

**Feature**: 003-core-logic
**Date**: 2026-03-19
**Stability**: Draft — awaiting approval

This document defines the complete public API surface of `src/core/`. The CLI layer
(`src/cli/`) is the only permitted caller. No other layer may import from `src/core/`
indirectly — and `src/core/` itself MUST NOT import from `src/cli/`.

---

## Error Type

```rust
// src/core/error.rs — re-exported as envy::core::CoreError
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

    /// The secret key name failed validation (empty or contains `=`).
    #[error("invalid secret key name \"{0}\": must be non-empty and must not contain `=`")]
    InvalidSecretKey(String),
}
```

---

## Manifest Type

```rust
// src/core/manifest.rs — re-exported as envy::core::Manifest
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    /// The UUID that links this directory tree to its vault entry.
    pub project_id: String,
}
```

---

## Constants

```rust
// src/core/ops.rs — re-exported as envy::core::DEFAULT_ENV
/// The environment name used when no explicit environment is provided.
pub const DEFAULT_ENV: &str = "development";
```

---

## Functions

### `find_manifest`

```rust
pub fn find_manifest(start_dir: &Path) -> Result<(Manifest, PathBuf), CoreError>
```

**Purpose**: Walks the directory tree upward from `start_dir` searching for `envy.toml`.
Returns the parsed manifest and the directory in which it was found.

**Caller contract**:
- Pass `std::env::current_dir()?` as `start_dir` for normal CLI use.
- Do not call on a directory the process lacks read permission for.

**Guarantees**:
- On `Ok`: `Manifest.project_id` is the non-empty UUID string read from `envy.toml`;
  `PathBuf` is the absolute path of the directory containing the manifest file.
- On `Err(ManifestNotFound)`: no `envy.toml` exists from `start_dir` up to the filesystem
  root. The user should be told to run `envy init`.
- On `Err(ManifestInvalid)`: a file was found but failed TOML parsing or is missing
  required fields. The inner string contains a human-readable parse error.
- On `Err(ManifestIo)`: the file could not be read (permissions, OS error).

**Algorithm**:
1. Check `<start_dir>/envy.toml`. If present, read and parse → return `(manifest, start_dir)`.
2. Move to `start_dir.parent()`. Repeat until root is reached.
3. If root reached with no file found → `ManifestNotFound`.

**Side effects**: none (read-only).

---

### `create_manifest`

```rust
pub fn create_manifest(target_dir: &Path, project_id: &str) -> Result<(), CoreError>
```

**Purpose**: Writes a new `envy.toml` in `target_dir` containing the given `project_id`.

**Caller contract**:
- `target_dir` must be an existing directory; the caller is responsible for creating it.
- `project_id` should be a valid UUID v4 string (e.g., from `Uuid::new_v4().to_string()`).
- Do not call if `envy.toml` already exists in `target_dir`; this function will fail rather
  than silently overwrite.

**Guarantees**:
- On `Ok`: `<target_dir>/envy.toml` now exists and contains `project_id = "<uuid>"` plus
  a human-readable comment.
- On `Err(ManifestIo)`: the file could not be written (already exists, permission denied,
  disk full, etc.). The inner string is a diagnostic message.

**Side effects**: creates one file on disk.

---

### `set_secret`

```rust
pub fn set_secret(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,
    key:        &str,
    plaintext:  &str,
) -> Result<(), CoreError>
```

**Purpose**: Encrypts `plaintext` and stores it in the vault under `(project_id, env_name, key)`.
Upsert semantics: if the key already exists, the old ciphertext is replaced.

**Caller contract**:
- `master_key` MUST be the value returned by `get_or_create_master_key()`.
- `key` MUST be non-empty and MUST NOT contain `=`.
- `env_name` may be empty; it is normalised to `DEFAULT_ENV` ("development") automatically.
- `plaintext` may be empty.

**Guarantees**:
- On `Ok`: the secret is durably stored; a subsequent `get_secret` with the same arguments
  returns the same `plaintext`.
- On `Err(InvalidSecretKey)`: `key` failed validation; no DB or crypto operation was
  attempted.
- On `Err(Crypto(...))`: encryption failed (should not occur with a valid key).
- On `Err(Db(...))`: the DB write failed.

**Auto-create behaviour**: if `env_name` (or `DEFAULT_ENV`) does not yet exist in the vault
for this project, the environment is created transparently before the secret is written.

**Side effects**: may create a new environment row; creates or updates a secret row.

---

### `get_secret`

```rust
pub fn get_secret(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,
    key:        &str,
) -> Result<Zeroizing<String>, CoreError>
```

**Purpose**: Fetches and decrypts the secret for `(project_id, env_name, key)`.
Returns the plaintext wrapped in `Zeroizing` (zeroed on drop).

**Caller contract**:
- `master_key` MUST be the same key used during the corresponding `set_secret` call.
- `key` MUST be non-empty and MUST NOT contain `=`.
- `env_name` may be empty (normalised to `DEFAULT_ENV`); however, if that environment does
  not exist, `Db(NotFound)` is returned — read operations do NOT auto-create environments.

**Guarantees**:
- On `Ok`: the inner `String` is the exact `plaintext` passed to the corresponding
  `set_secret`; its backing memory is zeroed when the `Zeroizing` value is dropped.
- On `Err(InvalidSecretKey)`: key failed validation; no DB access.
- On `Err(Db(NotFound))`: the key, environment, or project does not exist.
- On `Err(Crypto(DecryptionFailed))`: ciphertext could not be authenticated (wrong key or
  corruption).

**Side effects**: none.

---

### `list_secret_keys`

```rust
pub fn list_secret_keys(
    vault:      &Vault,
    project_id: &ProjectId,
    env_name:   &str,
) -> Result<Vec<String>, CoreError>
```

**Purpose**: Returns the key names for all secrets in the given environment, ordered
alphabetically. Does NOT decrypt values — safe to call without the master key.

**Caller contract**:
- `env_name` may be empty (normalised to `DEFAULT_ENV`).
- If the environment does not exist, `Db(NotFound)` is returned.

**Guarantees**:
- On `Ok`: a (possibly empty) `Vec<String>` of key names, sorted lexicographically.
- On `Err(Db(NotFound))`: environment or project does not exist.

**Side effects**: none (read-only).

---

### `delete_secret`

```rust
pub fn delete_secret(
    vault:      &Vault,
    project_id: &ProjectId,
    env_name:   &str,
    key:        &str,
) -> Result<(), CoreError>
```

**Purpose**: Permanently deletes the secret for `(project_id, env_name, key)`.

**Caller contract**:
- `key` MUST be non-empty and MUST NOT contain `=`.
- `env_name` may be empty (normalised to `DEFAULT_ENV`).

**Guarantees**:
- On `Ok`: the secret no longer exists; a subsequent `get_secret` returns `Db(NotFound)`.
- On `Err(InvalidSecretKey)`: key failed validation; no DB access.
- On `Err(Db(NotFound))`: the key, environment, or project does not exist; no mutation
  occurred.

**Side effects**: deletes one secret row from the database.

---

### `get_env_secrets`

```rust
pub fn get_env_secrets(
    vault:      &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name:   &str,
) -> Result<HashMap<String, Zeroizing<String>>, CoreError>
```

**Purpose**: Decrypts ALL secrets for the given environment and returns them as a
`HashMap<String, Zeroizing<String>>` ready for child-process environment injection.

**Caller contract**:
- `master_key` MUST be the same key used during every prior `set_secret` call for this
  vault. A wrong key causes every decryption to fail.
- `env_name` may be empty (normalised to `DEFAULT_ENV`).

**Guarantees**:
- On `Ok`: the map contains every key in the environment; each value is the decrypted
  plaintext wrapped in `Zeroizing` (zeroed when the map is dropped).
- An empty environment returns `Ok(HashMap::new())`, not an error.
- **Atomicity**: if any single decryption fails, the entire operation fails immediately and
  no partial map is returned. Partial plaintext is never accessible to the caller.
- On `Err(Db(NotFound))`: the environment or project does not exist (read operations do NOT
  auto-create environments).
- On `Err(Crypto(DecryptionFailed))`: at least one secret could not be decrypted.

**Side effects**: none (read-only after environment lookup).

---

## Invariants

1. **No plaintext on disk**: `src/core/` never writes plaintext values or key material to any
   file, database column, or log. The DB layer only ever receives ciphertext blobs from Core.
2. **No cross-layer imports**: `src/core/` MUST NOT import `src/cli/`. Permitted imports are
   `src/db/` and `src/crypto/` only.
3. **Key validation before I/O**: `validate_key` is the first step in every operation that
   accepts a key argument. No DB or crypto call is made for an invalid key name.
4. **Read operations never auto-create**: `get_secret`, `list_secret_keys`, `delete_secret`,
   and `get_env_secrets` return `Db(NotFound)` when the environment does not exist; they
   never create rows.
5. **Write operations may auto-create environments**: `set_secret` is the only function
   that transparently creates a missing environment before writing.
6. **Atomic bulk decrypt**: `get_env_secrets` returns all-or-nothing — never a partial map.
7. **Memory zeroing**: every decrypted value returned from Core is wrapped in `Zeroizing`;
   the master key reference is caller-owned and never cloned inside Core.

---

## Vault / Crypto Layer Mapping

| Core operation | DB layer call(s) | Crypto layer call(s) |
|---|---|---|
| `find_manifest` | none | none |
| `create_manifest` | none | none |
| `set_secret` | `get_environment_by_name`, [`create_environment`], `upsert_secret` | `encrypt` |
| `get_secret` | `get_environment_by_name`, `get_secret` | `decrypt` |
| `list_secret_keys` | `get_environment_by_name`, `list_secrets` | none |
| `delete_secret` | `get_environment_by_name`, `delete_secret` | none |
| `get_env_secrets` | `get_environment_by_name`, `list_secrets` | `decrypt` (×N) |
