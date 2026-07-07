//! Local audit trail for secret-touching actions (`envy audit`).
//!
//! Records *that* a key was set/read/deleted, and *when* — never the value.
//! This is intentionally scoped to the CRUD surface (`set`, `get`, `rm`,
//! `run`): sync/crypto operations (`encrypt`, `decrypt`, `rotate`) already
//! leave a trail via `envy.enc`'s git history and `sync_markers`, so adding
//! them here would duplicate signal for a much larger change surface.
//!
//! # Layer rules
//! - MUST NOT import from `crate::cli`.
//! - MAY import from `crate::db` only (no cryptographic operation needed).

use crate::db::{AuditLogRecord, ProjectId, Vault};

use super::CoreError;
use super::ops::normalize_env;

// ---------------------------------------------------------------------------
// AuditAction
// ---------------------------------------------------------------------------

/// The set of actions recorded in the local audit trail.
///
/// Mirrors the DB `CHECK(action IN (...))` constraint in `db::schema` — this
/// enum is the first line of defense, the DB constraint is the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    Set,
    Get,
    Rm,
    Run,
}

impl AuditAction {
    /// Lowercase string form stored in the `audit_logs.action` column.
    pub fn as_str(self) -> &'static str {
        match self {
            AuditAction::Set => "set",
            AuditAction::Get => "get",
            AuditAction::Rm => "rm",
            AuditAction::Run => "run",
        }
    }
}

// ---------------------------------------------------------------------------
// AuditEntry — human-facing DTO for `envy audit`
// ---------------------------------------------------------------------------

/// One row of the audit trail, ready for display.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Lowercase environment label the action was performed against.
    pub environment: String,
    /// The recorded action (see [`AuditAction`]).
    pub action: String,
    /// Secret key name touched, or `None` for whole-environment actions (`run`).
    pub key: Option<String>,
    /// Unix epoch (UTC, seconds) the action was recorded.
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Records one audit entry for `(project_id, env_name)`.
///
/// `env_name` is normalised exactly like every other Core operation (empty →
/// [`super::DEFAULT_ENV`], then lowercased) so the audit trail always lines
/// up with the environment the primary operation actually touched.
///
/// Callers (CLI command handlers) treat a failure here as best-effort: the
/// primary operation (e.g. `envy set`) has already succeeded by the time this
/// is called, and a broken audit trail must not make secret management
/// unusable. Callers should warn on stderr rather than abort the command.
///
/// # Errors
/// - [`CoreError::Db`] if the environment cannot be resolved or the insert fails.
pub fn record(
    vault: &Vault,
    project_id: &ProjectId,
    env_name: &str,
    action: AuditAction,
    key: Option<&str>,
) -> Result<(), CoreError> {
    let name = normalize_env(env_name);
    let env = vault.get_environment_by_name(project_id, &name)?;
    vault.insert_audit_log(&env.id, action.as_str(), key)?;
    Ok(())
}

/// Returns the most recent audit entries for `project_id`, newest first.
///
/// `env_filter` restricts the report to a single environment. `limit` caps
/// the number of rows (the audit trail is append-only and unbounded).
///
/// # Errors
/// - [`CoreError::Db`] for unexpected SQLite errors.
pub fn list_audit(
    vault: &Vault,
    project_id: &ProjectId,
    env_filter: Option<&str>,
    limit: i64,
) -> Result<Vec<AuditEntry>, CoreError> {
    let normalized_filter = env_filter.map(|s| s.to_lowercase());
    let rows: Vec<AuditLogRecord> =
        vault.list_audit_logs(project_id, normalized_filter.as_deref(), limit)?;

    Ok(rows
        .into_iter()
        .map(|r| AuditEntry {
            environment: r.environment_name,
            action: r.action,
            key: r.key,
            created_at: r.created_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0xABu8; 32];

    fn open_test_vault() -> (tempfile::TempDir, Vault, ProjectId) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("vault.db");
        let vault = Vault::open(&path, &TEST_KEY).expect("vault open");
        let pid = vault
            .create_project("test-project")
            .expect("create project");
        (tmp, vault, pid)
    }

    #[test]
    fn record_and_list_round_trip() {
        let (_tmp, vault, pid) = open_test_vault();
        vault
            .create_environment(&pid, "development")
            .expect("create env");

        record(&vault, &pid, "development", AuditAction::Set, Some("API_KEY"))
            .expect("record must succeed");

        let entries = list_audit(&vault, &pid, None, 10).expect("list must succeed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "set");
        assert_eq!(entries[0].key.as_deref(), Some("API_KEY"));
        assert_eq!(entries[0].environment, "development");
    }

    #[test]
    fn record_normalizes_empty_env_name_to_default() {
        let (_tmp, vault, pid) = open_test_vault();
        vault
            .create_environment(&pid, crate::core::DEFAULT_ENV)
            .expect("create env");

        record(&vault, &pid, "", AuditAction::Get, Some("KEY")).expect("record must succeed");

        let entries = list_audit(&vault, &pid, None, 10).expect("list must succeed");
        assert_eq!(entries[0].environment, crate::core::DEFAULT_ENV);
    }

    #[test]
    fn list_audit_filter_is_case_insensitive() {
        let (_tmp, vault, pid) = open_test_vault();
        vault
            .create_environment(&pid, "production")
            .expect("create env");
        record(&vault, &pid, "production", AuditAction::Run, None).expect("record must succeed");

        let entries =
            list_audit(&vault, &pid, Some("PRODUCTION"), 10).expect("list must succeed");
        assert_eq!(entries.len(), 1, "filter must normalize case like env names do");
    }

    #[test]
    fn run_action_has_no_key() {
        let (_tmp, vault, pid) = open_test_vault();
        vault
            .create_environment(&pid, "development")
            .expect("create env");
        record(&vault, &pid, "development", AuditAction::Run, None).expect("record must succeed");

        let entries = list_audit(&vault, &pid, None, 10).expect("list must succeed");
        assert_eq!(entries[0].key, None);
    }
}
