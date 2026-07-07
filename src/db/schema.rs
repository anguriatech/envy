//! Schema migration runner.
//!
//! Migration versions are tracked via `PRAGMA user_version`.
//! All migrations are additive — existing tables are never altered or dropped.
//!
//! Current versions:
//!   0 → 1: Initial schema (projects, environments, secrets).
//!   1 → 2: Add sync_markers table (sealed_at per environment).
//!   2 → 3: Add audit_logs table (local, append-only action history).

use super::error::{DbError, is_encryption_error};

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

/// DDL for schema version 2.
///
/// Adds the `sync_markers` table which records the Unix timestamp of the last
/// successful `envy encrypt` operation per environment. Used by `envy status`
/// to compute In Sync / Modified / Never Sealed without decrypting any secrets.
const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS sync_markers (
    -- UUID of the sealed environment (FK to environments.id).
    -- ON DELETE CASCADE ensures rows are cleaned up when the environment is deleted.
    environment_id  TEXT    NOT NULL PRIMARY KEY
                            REFERENCES environments(id) ON DELETE CASCADE,

    -- Unix epoch (UTC, seconds) of the last successful seal for this environment.
    -- Updated via INSERT OR REPLACE on every successful envy encrypt.
    sealed_at       INTEGER NOT NULL
);
";

/// DDL for schema version 3.
///
/// Adds the `audit_logs` table: a local, append-only record of secret-touching
/// actions (`set`, `get`, `rm`, `run`) for `envy audit`. Rows are never updated,
/// only inserted — this is a forensic trail, not a mutable projection.
///
/// # Security contract
/// `value_encrypted` and secret values are NEVER written to this table — only
/// the action name, the key name (nullable for whole-environment actions),
/// and a timestamp. A leaked or corrupted vault must not turn this table into
/// a second copy of the secrets themselves.
const SCHEMA_V3: &str = "
CREATE TABLE IF NOT EXISTS audit_logs (
    -- Globally unique identifier (UUID v4, hyphenated).
    id              TEXT    NOT NULL PRIMARY KEY
                            CHECK(length(id) = 36),

    -- Environment the action was performed against. CASCADE ensures audit
    -- rows are cleaned up when the environment itself is deleted (they can
    -- no longer be attributed to anything the user can inspect).
    environment_id  TEXT    NOT NULL
                            REFERENCES environments(id) ON DELETE CASCADE,

    -- One of: set, get, rm, run. Enforced at the DB level as a second line
    -- of defense after the Core layer's AuditAction enum.
    action          TEXT    NOT NULL
                            CHECK(action IN ('set', 'get', 'rm', 'run')),

    -- Secret key name touched by the action. NULL for whole-environment
    -- actions (e.g. 'run', which touches every secret in the environment).
    key             TEXT,

    -- Unix epoch (UTC, seconds). Set once on INSERT; rows are never updated.
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_environment_created
    ON audit_logs(environment_id, created_at DESC);
";

/// Checks the current `user_version` and applies any pending migrations.
///
/// - If `user_version` is 0 (new vault): creates all V1 tables and sets version to 1.
/// - If `user_version` is 1: creates the `sync_markers` table and sets version to 2.
/// - If `user_version` is >= 2: a no-op (future versions will add incremental steps).
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

    // Re-read the version after the V1 step so a fresh vault (0 → 1 → 2)
    // also gets the V2 migration in the same open call.
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| DbError::MigrationError(e.to_string()))?;

    if version == 1 {
        conn.execute_batch(SCHEMA_V2)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;

        conn.pragma_update(None, "user_version", 2i64)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;
    }

    // Re-read again so a fresh vault (0 → 1 → 2 → 3) also gets V3 in one call.
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| DbError::MigrationError(e.to_string()))?;

    if version == 2 {
        conn.execute_batch(SCHEMA_V3)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;

        conn.pragma_update(None, "user_version", 3i64)
            .map_err(|e| DbError::MigrationError(e.to_string()))?;
    }

    Ok(())
}
