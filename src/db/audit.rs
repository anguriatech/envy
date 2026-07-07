//! Audit log persistence — append-only history of secret-touching actions.
//!
//! # Layer rules
//! - This file MUST NOT import from `crate::cli`, `crate::core`, or `crate::crypto`.
//! - All functions return `Result<T, DbError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.
//!
//! # Security contract
//! - Secret values are NEVER written here — only action names, key names, and
//!   timestamps. This table must never become a second copy of the secrets.
//! - `action` is validated against a fixed set by the DB `CHECK` constraint;
//!   the Core layer's `AuditAction` enum is the first line of defense.

use rusqlite::params;
use uuid::Uuid;

use super::{
    EnvId, ProjectId, Vault,
    error::{DbError, map_rusqlite_error},
};

/// One row of the local audit trail, joined with the environment name for
/// display (callers should not need a second lookup to render a report).
#[derive(Debug, Clone)]
pub struct AuditLogRecord {
    /// Globally unique identifier (UUID v4, hyphenated).
    pub id: String,
    /// Lowercase environment label the action was performed against.
    pub environment_name: String,
    /// One of `set`, `get`, `rm`, `run` (validated by the DB `CHECK` constraint).
    pub action: String,
    /// Secret key name touched, or `None` for whole-environment actions (`run`).
    pub key: Option<String>,
    /// Unix epoch (UTC, seconds) the action was recorded.
    pub created_at: i64,
}

impl Vault {
    /// Appends one row to the audit trail for `env_id`.
    ///
    /// `action` MUST be one of `set`, `get`, `rm`, `run` — the DB `CHECK`
    /// constraint rejects anything else with `DbError::ConstraintViolation`.
    /// `key` is `None` for whole-environment actions.
    ///
    /// # Errors
    /// - [`DbError::ConstraintViolation`] if `action` is not a recognised value
    ///   or `env_id` does not reference an existing environment (FK violation).
    pub fn insert_audit_log(
        &self,
        env_id: &EnvId,
        action: &str,
        key: Option<&str>,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO audit_logs (id, environment_id, action, key)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, env_id.as_str(), action, key],
            )
            .map_err(map_rusqlite_error)?;
        Ok(())
    }

    /// Returns the most recent audit log rows for `project_id`, newest first.
    ///
    /// `env_name_filter` restricts the report to a single environment when
    /// `Some`. `limit` caps the number of rows returned (the table is
    /// append-only and can grow indefinitely over a project's lifetime).
    ///
    /// Returns an empty `Vec` if the project has no audit history yet.
    ///
    /// # Errors
    /// - [`DbError::Internal`] for unexpected SQLite errors.
    pub fn list_audit_logs(
        &self,
        project_id: &ProjectId,
        env_name_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditLogRecord>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT a.id, e.name, a.action, a.key, a.created_at
                 FROM audit_logs a
                 JOIN environments e ON e.id = a.environment_id
                 WHERE e.project_id = ?1
                   AND (?2 IS NULL OR e.name = ?2)
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT ?3",
            )
            .map_err(map_rusqlite_error)?;

        let rows = stmt
            .query_map(
                params![project_id.as_str(), env_name_filter, limit],
                |row| {
                    Ok(AuditLogRecord {
                        id: row.get(0)?,
                        environment_name: row.get(1)?,
                        action: row.get(2)?,
                        key: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .map_err(map_rusqlite_error)?;

        rows.map(|r| r.map_err(map_rusqlite_error))
            .collect::<Result<Vec<AuditLogRecord>, DbError>>()
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

    #[test]
    fn schema_v3_migration_adds_audit_logs_table() {
        let (_tmp, vault, _pid) = open_vault();
        assert!(
            vault
                .table_exists("audit_logs")
                .expect("table_exists must not fail"),
            "audit_logs table must exist after V3 migration"
        );
    }

    #[test]
    fn insert_and_list_audit_log_round_trip() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        vault
            .insert_audit_log(&env_id, "set", Some("API_KEY"))
            .expect("insert must succeed");

        let rows = vault
            .list_audit_logs(&pid, None, 10)
            .expect("list must succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].environment_name, "development");
        assert_eq!(rows[0].action, "set");
        assert_eq!(rows[0].key.as_deref(), Some("API_KEY"));
    }

    #[test]
    fn insert_audit_log_allows_null_key_for_whole_env_actions() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        vault
            .insert_audit_log(&env_id, "run", None)
            .expect("insert with null key must succeed");

        let rows = vault
            .list_audit_logs(&pid, None, 10)
            .expect("list must succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, None);
    }

    #[test]
    fn insert_audit_log_rejects_unknown_action() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        let result = vault.insert_audit_log(&env_id, "delete-everything", None);
        assert!(
            matches!(result, Err(DbError::ConstraintViolation(_))),
            "unknown action must be rejected by the CHECK constraint, got: {:?}",
            result
        );
    }

    #[test]
    fn list_audit_logs_orders_newest_first() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        vault
            .insert_audit_log(&env_id, "set", Some("FIRST"))
            .expect("insert 1");
        std::thread::sleep(std::time::Duration::from_secs(1));
        vault
            .insert_audit_log(&env_id, "set", Some("SECOND"))
            .expect("insert 2");

        let rows = vault
            .list_audit_logs(&pid, None, 10)
            .expect("list must succeed");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].key.as_deref(),
            Some("SECOND"),
            "most recent row must come first"
        );
    }

    #[test]
    fn list_audit_logs_filters_by_environment() {
        let (_tmp, vault, pid) = open_vault();
        let dev_id = vault
            .create_environment(&pid, "development")
            .expect("create dev");
        let prod_id = vault
            .create_environment(&pid, "production")
            .expect("create prod");

        vault
            .insert_audit_log(&dev_id, "set", Some("DEV_KEY"))
            .expect("insert dev");
        vault
            .insert_audit_log(&prod_id, "set", Some("PROD_KEY"))
            .expect("insert prod");

        let rows = vault
            .list_audit_logs(&pid, Some("production"), 10)
            .expect("list must succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].environment_name, "production");
        assert_eq!(rows[0].key.as_deref(), Some("PROD_KEY"));
    }

    #[test]
    fn list_audit_logs_respects_limit() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");

        for i in 0..5 {
            vault
                .insert_audit_log(&env_id, "set", Some(&format!("KEY_{i}")))
                .expect("insert must succeed");
        }

        let rows = vault
            .list_audit_logs(&pid, None, 3)
            .expect("list must succeed");
        assert_eq!(rows.len(), 3, "limit must cap the number of returned rows");
    }

    #[test]
    fn audit_logs_deleted_on_environment_cascade() {
        let (_tmp, vault, pid) = open_vault();
        let env_id = vault
            .create_environment(&pid, "development")
            .expect("create env");
        vault
            .insert_audit_log(&env_id, "set", Some("KEY"))
            .expect("insert must succeed");

        vault
            .delete_environment(&env_id)
            .expect("delete environment");

        let count: i64 = vault
            .conn
            .query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))
            .expect("count must succeed");
        assert_eq!(
            count, 0,
            "audit_logs must be deleted by CASCADE when env is deleted"
        );
    }
}
