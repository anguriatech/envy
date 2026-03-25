//! Sync marker CRUD and environment status aggregation.
//!
//! A sync marker records the Unix timestamp of the last successful `envy encrypt`
//! operation for one environment. It is the sole data source for `envy status`
//! sync-state computation — no secret values are ever read here.
//!
//! # Layer rules
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::crypto`.
//! - All functions return `Result<T, DbError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.

use rusqlite::params;

use super::{
    EnvId, ProjectId, Vault,
    error::{DbError, map_rusqlite_error},
};

// ---------------------------------------------------------------------------
// EnvironmentStatus — raw aggregate returned by the DB aggregation query
// ---------------------------------------------------------------------------

/// Raw per-environment data returned by [`Vault::environment_status`].
///
/// This is a plain data transfer object. The Core layer is responsible for
/// deriving the human-readable sync state (`SyncStatus`) from these fields.
#[derive(Debug, Clone)]
pub struct EnvironmentStatus {
    /// Lowercase environment label (e.g., `development`, `production`).
    pub name: String,

    /// Number of secrets currently stored for this environment.
    pub secret_count: i64,

    /// Unix epoch (UTC, seconds) of the most-recently modified secret.
    /// `None` when the environment has zero secrets.
    pub last_modified_at: Option<i64>,

    /// Unix epoch (UTC, seconds) of the last successful seal operation.
    /// `None` when the environment has never been encrypted.
    pub sealed_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// Vault impl
// ---------------------------------------------------------------------------

impl Vault {
    /// Records (or updates) the seal timestamp for `env_id`.
    ///
    /// Uses `INSERT OR REPLACE` so the first call creates the row and every
    /// subsequent call overwrites `sealed_at` with the new timestamp.
    ///
    /// # Errors
    /// - [`DbError::ConstraintViolation`] if `env_id` does not reference an
    ///   existing environment (FK violation).
    /// - [`DbError::Internal`] for unexpected SQLite errors.
    pub fn upsert_sync_marker(&self, env_id: &EnvId, sealed_at: i64) -> Result<(), DbError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO sync_markers (environment_id, sealed_at)
                 VALUES (?1, ?2)",
                params![env_id.as_str(), sealed_at],
            )
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Returns aggregate sync-state data for every environment in `project_id`,
    /// ordered alphabetically by environment name.
    ///
    /// The query joins `environments`, `secrets`, and `sync_markers` in a single
    /// pass so that all data is fetched in one round-trip. No secret *values* are
    /// read — only `updated_at` timestamps and row counts.
    ///
    /// Returns an empty `Vec` if the project has no environments.
    ///
    /// # Errors
    /// - [`DbError::Internal`] for unexpected SQLite errors.
    pub fn environment_status(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<EnvironmentStatus>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT
                     e.name,
                     COUNT(s.id)       AS secret_count,
                     MAX(s.updated_at) AS last_modified_at,
                     sm.sealed_at
                 FROM environments e
                 LEFT JOIN secrets s
                       ON  s.environment_id = e.id
                 LEFT JOIN sync_markers sm
                       ON  sm.environment_id = e.id
                 WHERE e.project_id = ?1
                 GROUP BY e.id, e.name, sm.sealed_at
                 ORDER BY e.name ASC",
            )
            .map_err(map_rusqlite_error)?;

        let rows = stmt
            .query_map(params![project_id.as_str()], |row| {
                Ok(EnvironmentStatus {
                    name: row.get(0)?,
                    secret_count: row.get(1)?,
                    last_modified_at: row.get(2)?,
                    sealed_at: row.get(3)?,
                })
            })
            .map_err(map_rusqlite_error)?;

        rows.map(|r| r.map_err(map_rusqlite_error))
            .collect::<Result<Vec<EnvironmentStatus>, DbError>>()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0xABu8; 32];

    fn open_vault() -> (tempfile::TempDir, Vault, crate::db::ProjectId) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("vault.db");
        let vault = Vault::open(&path, &TEST_KEY).expect("vault open");
        let pid = vault
            .create_project("test-project")
            .expect("create project");
        (tmp, vault, pid)
    }

    // T007 — V2 migration creates the sync_markers table
    #[test]
    fn schema_v2_migration_adds_sync_markers_table() {
        let (_tmp, vault, _pid) = open_vault();
        assert!(
            vault
                .table_exists("sync_markers")
                .expect("table_exists must not fail"),
            "sync_markers table must exist after V2 migration"
        );
    }

    // T008 — upsert creates a row
    #[test]
    fn upsert_sync_marker_creates_row() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        vault
            .upsert_sync_marker(&env_id, 1_000)
            .expect("upsert must succeed");

        let sealed_at: i64 = vault
            .conn
            .query_row(
                "SELECT sealed_at FROM sync_markers WHERE environment_id = ?1",
                params![env_id.as_str()],
                |row| row.get(0),
            )
            .expect("row must exist");
        assert_eq!(sealed_at, 1_000);
    }

    // T009 — upsert overwrites an existing row (only one row, updated value)
    #[test]
    fn upsert_sync_marker_updates_existing_row() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        vault
            .upsert_sync_marker(&env_id, 1_000)
            .expect("first upsert");
        vault
            .upsert_sync_marker(&env_id, 2_000)
            .expect("second upsert");

        let count: i64 = vault
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sync_markers WHERE environment_id = ?1",
                params![env_id.as_str()],
                |row| row.get(0),
            )
            .expect("count must succeed");
        assert_eq!(count, 1, "only one row must exist after two upserts");

        let sealed_at: i64 = vault
            .conn
            .query_row(
                "SELECT sealed_at FROM sync_markers WHERE environment_id = ?1",
                params![env_id.as_str()],
                |row| row.get(0),
            )
            .expect("row must exist");
        assert_eq!(
            sealed_at, 2_000,
            "sealed_at must be updated to the latest value"
        );
    }

    // T010 — environment_status returns None sealed_at when no marker exists
    #[test]
    fn environment_status_returns_never_sealed_when_no_marker() {
        let (_tmp, vault, pid) = open_vault();
        vault
            .create_environment(&pid, "development")
            .expect("create env");
        // Add a secret so secret_count is 1 (no crypto needed — we insert raw bytes).
        let env_id = vault
            .get_environment_by_name(&pid, "development")
            .expect("env must exist")
            .id;
        vault
            .upsert_secret(&env_id, "MY_KEY", &[0u8; 32], &[0u8; 12])
            .expect("upsert secret");

        let statuses = vault.environment_status(&pid).expect("env_status");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].secret_count, 1);
        assert!(
            statuses[0].sealed_at.is_none(),
            "sealed_at must be None when no sync marker exists"
        );
    }

    // T011 — environment_status returns correct last_modified_at
    #[test]
    fn environment_status_returns_correct_last_modified_at() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");
        vault
            .upsert_secret(&env_id, "KEY", &[0u8; 32], &[0u8; 12])
            .expect("upsert secret");

        let statuses = vault.environment_status(&pid).expect("env_status");
        assert_eq!(statuses.len(), 1);
        assert!(
            statuses[0].last_modified_at.is_some(),
            "last_modified_at must be Some when secrets exist"
        );
    }

    // T012 — environment_status returns None last_modified_at for empty envs
    #[test]
    fn environment_status_zero_secrets_returns_none_last_modified() {
        let (_tmp, vault, pid) = open_vault();
        vault
            .create_environment(&pid, "empty-env")
            .expect("create env");

        let statuses = vault.environment_status(&pid).expect("env_status");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].secret_count, 0);
        assert!(
            statuses[0].last_modified_at.is_none(),
            "last_modified_at must be None when there are no secrets"
        );
    }

    // T013 — environment_status returns rows sorted by name ascending
    #[test]
    fn environment_status_returns_multiple_envs_sorted_by_name() {
        let (_tmp, vault, pid) = open_vault();
        vault
            .create_environment(&pid, "zebra")
            .expect("create zebra");
        vault
            .create_environment(&pid, "alpha")
            .expect("create alpha");

        let statuses = vault.environment_status(&pid).expect("env_status");
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].name, "alpha");
        assert_eq!(statuses[1].name, "zebra");
    }

    // T014 — sync_marker is deleted when its parent environment is deleted (CASCADE)
    #[test]
    fn sync_marker_deleted_on_environment_cascade() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");
        vault
            .upsert_sync_marker(&env_id, 1_000)
            .expect("upsert marker");

        vault
            .delete_environment(&env_id)
            .expect("delete environment");

        let count: i64 = vault
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sync_markers WHERE environment_id = ?1",
                params![env_id.as_str()],
                |row| row.get(0),
            )
            .expect("count must succeed");
        assert_eq!(
            count, 0,
            "sync_marker must be deleted by CASCADE when env is deleted"
        );
    }
}
