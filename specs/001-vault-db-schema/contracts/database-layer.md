# Contract: Database Layer Public Interface

**Layer**: Database (rusqlite + bundled-sqlcipher)
**Consumed by**: Core/Business Logic layer exclusively
**Date**: 2026-03-18

---

## Overview

This document defines the operations the Database layer MUST expose to the Core layer.
The Core layer MUST NOT write raw SQL — it calls these operations only. The Database layer
MUST NOT call into the Core or UI/CLI layers (dependency flow: Core → Database, never
the reverse).

All operations return `Result<T, DbError>`. A typed `DbError` enum (defined in this layer)
is the only error type the Core layer receives from here.

---

## Connection Contract

```
Vault::open(vault_path: &Path, master_key: &[u8]) -> Result<Vault, DbError>
```

- Opens (or creates) the SQLCipher-encrypted vault at `vault_path`.
- Applies the master key, sets `PRAGMA foreign_keys = ON` and `PRAGMA journal_mode = WAL`.
- Runs schema migrations automatically (checks `user_version`, creates tables if needed).
- The `master_key` byte slice MUST be zeroed by the caller after this call returns.

```
Vault::close(self) -> Result<(), DbError>
```

- Flushes WAL and closes the connection cleanly.

---

## Project Operations

```
Vault::create_project(name: &str) -> Result<ProjectId, DbError>
```
- Generates a new UUID v4, inserts into `projects`.
- Returns the new `ProjectId` (newtype over `String`).

```
Vault::get_project(id: &ProjectId) -> Result<Project, DbError>
```
- Returns the `Project` record or `DbError::NotFound`.

```
Vault::get_project_by_name(name: &str) -> Result<Project, DbError>
```
- Returns the first matching project by name, or `DbError::NotFound`.

```
Vault::list_projects() -> Result<Vec<Project>, DbError>
```
- Returns all projects ordered by `created_at ASC`.

```
Vault::delete_project(id: &ProjectId) -> Result<(), DbError>
```
- Deletes the project. CASCADE removes all environments and their secrets.

---

## Environment Operations

```
Vault::create_environment(project_id: &ProjectId, name: &str) -> Result<EnvId, DbError>
```
- `name` MUST already be lowercased by the caller before this call.
- Returns `DbError::AlreadyExists` if `(project_id, name)` already exists.

```
Vault::get_environment(id: &EnvId) -> Result<Environment, DbError>
```

```
Vault::get_environment_by_name(
    project_id: &ProjectId,
    name: &str,
) -> Result<Environment, DbError>
```
- Case-sensitive lookup (caller MUST normalize to lowercase first).

```
Vault::list_environments(project_id: &ProjectId) -> Result<Vec<Environment>, DbError>
```

```
Vault::delete_environment(id: &EnvId) -> Result<(), DbError>
```
- CASCADE removes all secrets in this environment.

---

## Secret Operations

```
Vault::upsert_secret(
    env_id:           &EnvId,
    key:              &str,
    value_encrypted:  &[u8],  -- AES-256-GCM ciphertext (produced by Crypto layer)
    value_nonce:      &[u8],  -- 12-byte nonce (produced by Crypto layer)
) -> Result<SecretId, DbError>
```
- Inserts or replaces (`INSERT OR REPLACE`) the secret for `(env_id, key)`.
- The Database layer MUST NOT encrypt or decrypt — it stores and retrieves opaque bytes.
- Updates `updated_at` on replace.

```
Vault::get_secret(
    env_id: &EnvId,
    key:    &str,
) -> Result<SecretRecord, DbError>
```
- Returns `SecretRecord { id, key, value_encrypted, value_nonce, created_at, updated_at }`.
- Returns `DbError::NotFound` if no matching row.

```
Vault::list_secrets(env_id: &EnvId) -> Result<Vec<SecretRecord>, DbError>
```
- Returns all secrets for the environment, ordered by `key ASC`.
- `value_encrypted` and `value_nonce` are included; the Core layer decrypts.

```
Vault::delete_secret(env_id: &EnvId, key: &str) -> Result<(), DbError>
```
- Returns `DbError::NotFound` if the key does not exist.

---

## Error Type

```
enum DbError {
    NotFound,           -- Requested record does not exist
    AlreadyExists,      -- Unique constraint violation (environment name duplicate, etc.)
    ConstraintViolation(String),  -- Other constraint violations
    IoError(String),    -- Filesystem/file access errors
    EncryptionError,    -- Wrong master key (SQLCipher PRAGMA key rejected)
    MigrationError(String),  -- Schema migration failed
    Internal(String),   -- Catch-all for unexpected rusqlite errors
}
```

---

## Invariants (the Core layer may rely on these)

1. A `SecretRecord` returned by `get_secret` or `list_secrets` ALWAYS contains
   `value_encrypted` and `value_nonce` as non-empty byte slices.
2. `upsert_secret` is atomic: either the full row is written or nothing is (no partial
   writes possible).
3. Cascade deletes are always enforced (`PRAGMA foreign_keys = ON` is set on every
   connection open).
4. `value_nonce` is always exactly 12 bytes — enforced by the DB `CHECK` constraint.
5. The Database layer NEVER logs or prints `value_encrypted`, `value_nonce`, or any
   argument named `master_key`.
