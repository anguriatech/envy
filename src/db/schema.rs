//! Schema migration runner.
//!
//! Migration versions are tracked via `PRAGMA user_version`.
//! All migrations are additive — existing tables are never altered or dropped.
//!
//! Current versions:
//!   0 → 1: Initial schema (projects, environments, secrets).

use super::error::{is_encryption_error, DbError};

/// Full DDL for schema version 1.
///
/// Tables are created with `IF NOT EXISTS` so that calling this inside a retry
/// or after a partial migration is always safe.
const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS projects (
    -- Globally unique project identifier (UUID v4, hyphenated TEXT).
    -- Stable across machines; FK anchor for environments and future users/roles.
    id          TEXT    NOT NULL PRIMARY KEY
                        CHECK(length(id) = 36),

    -- Human-readable project name (e.g., directory name or user-supplied label).
    name        TEXT    NOT NULL
                        CHECK(length(name) > 0),

    -- Unix epoch (UTC, seconds). Set once on INSERT; never updated.
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- Unix epoch (UTC, seconds). Updated on every modification to this row.
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE TABLE IF NOT EXISTS environments (
    -- Globally unique environment identifier (UUID v4, hyphenated TEXT).
    id          TEXT    NOT NULL PRIMARY KEY
                        CHECK(length(id) = 36),

    -- Parent project. CASCADE ensures no orphaned environments survive project deletion.
    project_id  TEXT    NOT NULL
                        REFERENCES projects(id) ON DELETE CASCADE,

    -- Environment label normalized to lowercase before INSERT.
    -- The CHECK is a DB-level guard after application-side normalization.
    name        TEXT    NOT NULL
                        CHECK(name = lower(name))
                        CHECK(length(name) > 0),

    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- One environment name per project.
    UNIQUE(project_id, name)
);

CREATE TABLE IF NOT EXISTS secrets (
    -- Globally unique secret identifier (UUID v4, hyphenated TEXT).
    -- Stable FK target for future audit_logs in Phase 3.
    id                  TEXT    NOT NULL PRIMARY KEY
                                CHECK(length(id) = 36),

    -- Parent environment. CASCADE ensures no orphaned secrets on environment deletion.
    environment_id      TEXT    NOT NULL
                                REFERENCES environments(id) ON DELETE CASCADE,

    -- Secret key name (e.g., DATABASE_URL, STRIPE_KEY).
    -- Format validation (uppercase, underscores) is the CLI layer's responsibility.
    key                 TEXT    NOT NULL
                                CHECK(length(key) > 0),

    -- Defense-in-depth layer 2: AES-256-GCM ciphertext.
    -- The DB layer stores and returns these bytes verbatim — it never decrypts.
    value_encrypted     BLOB    NOT NULL,

    -- 12-byte (96-bit) random nonce for AES-256-GCM.
    -- Unique per row so that identical values produce different ciphertexts.
    value_nonce         BLOB    NOT NULL
                                CHECK(length(value_nonce) = 12),

    created_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- One value per key per environment. INSERT OR REPLACE against this constraint
    -- implements the atomic overwrite behavior for 'envy set'.
    UNIQUE(environment_id, key)
);
";

/// Checks the current `user_version` and applies any pending migrations.
///
/// - If `user_version` is 0 (new vault): creates all tables and sets version to 1.
/// - If `user_version` is >= 1: a no-op (future versions will add incremental steps).
///
/// Any SQL error during the version read is checked for `SQLITE_NOTADB` (26) and
/// mapped to `DbError::EncryptionError` — the most common cause of that error is a
/// wrong master key.
pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), DbError> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| {
            if is_encryption_error(&e) {
                DbError::EncryptionError
            } else {
                DbError::MigrationError(e.to_string())
            }
        })?;

    if version == 0 {
        conn.execute_batch(SCHEMA_V1)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;

        conn.pragma_update(None, "user_version", 1i64)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;
    }

    Ok(())
}
