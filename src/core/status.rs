//! Sync-state derivation for the `envy status` command.
//!
//! This module reads raw per-environment aggregate data from the DB layer and
//! derives a human-readable [`SyncStatus`] for each environment. No secret
//! values are ever read here.
//!
//! # Layer rules
//! - MUST NOT import from `crate::cli` or `crate::crypto`.
//! - All functions return `Result<T, CoreError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.

use crate::db::{ProjectId, Vault};

use super::error::CoreError;

// ---------------------------------------------------------------------------
// SyncStatus — human-readable derivation
// ---------------------------------------------------------------------------

/// The sync state of a single environment derived from DB timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// All secrets were last modified before (or at the same time as) the last
    /// successful seal. The vault and `envy.enc` are in sync.
    InSync,

    /// At least one secret was modified after the last seal. The environment
    /// needs to be re-encrypted.
    Modified,

    /// The environment has never been sealed. No entry exists in `sync_markers`.
    NeverSealed,
}

// ---------------------------------------------------------------------------
// StatusRow — per-environment DTO returned by get_status_report
// ---------------------------------------------------------------------------

/// Per-environment data returned by [`get_status_report`].
///
/// Combines the raw [`EnvironmentStatus`] from the DB with the derived
/// [`SyncStatus`] computed by this module.
#[derive(Debug, Clone)]
pub struct StatusRow {
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

    /// The derived sync state for this environment.
    pub sync_status: SyncStatus,

    /// Names of secrets whose `updated_at` is older than the manifest's
    /// `rotation_reminder_days` threshold. Empty when nothing is stale.
    /// Never includes secret values — only key names, safe to print.
    pub stale_secrets: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Derives the [`SyncStatus`] for one environment from raw DB timestamps.
///
/// Rules (Constitution Principle: DB returns numbers; Core derives state):
/// - `sealed_at` is `None` → [`SyncStatus::NeverSealed`]
/// - `last_modified_at > sealed_at` → [`SyncStatus::Modified`]
/// - otherwise → [`SyncStatus::InSync`]
///
/// An environment with zero secrets (`last_modified_at` is `None`) and a
/// sync marker (`sealed_at` is `Some`) is treated as [`SyncStatus::InSync`]:
/// there is nothing newer than the last seal.
pub fn derive_sync_status(last_modified_at: Option<i64>, sealed_at: Option<i64>) -> SyncStatus {
    match sealed_at {
        None => SyncStatus::NeverSealed,
        Some(sealed) => match last_modified_at {
            Some(modified) if modified > sealed => SyncStatus::Modified,
            _ => SyncStatus::InSync,
        },
    }
}

/// Returns the key names of secrets in `env_id` whose `updated_at` is older
/// than `threshold_days` days (measured against the current wall-clock time).
///
/// Reads only key names and timestamps via [`Vault::list_secrets`] — never
/// decrypts a value, so this is safe to call from `envy status` (which must
/// not touch ciphertext or prompt for a passphrase).
///
/// A `threshold_days` of `0` disables the reminder entirely (returns empty).
fn stale_secret_keys(
    vault: &Vault,
    env_id: &crate::db::EnvId,
    threshold_days: u32,
) -> Result<Vec<String>, CoreError> {
    if threshold_days == 0 {
        return Ok(Vec::new());
    }
    let threshold_secs = i64::from(threshold_days) * 86_400;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Ok(vault
        .list_secrets(env_id)?
        .into_iter()
        .filter(|s| now.saturating_sub(s.updated_at) > threshold_secs)
        .map(|s| s.key)
        .collect())
}

/// Returns a [`StatusRow`] for every environment in `project_id`, ordered
/// alphabetically by environment name.
///
/// Calls [`Vault::environment_status`] in a single DB round-trip, then maps
/// each row through [`derive_sync_status`]. A second per-environment query
/// via [`stale_secret_keys`] computes the rotation reminder — negligible cost
/// at the secret counts this tool is designed for (tens to low hundreds).
///
/// Returns an empty `Vec` if the project has no environments.
///
/// # Errors
/// - [`CoreError::Database`] for unexpected SQLite errors.
pub fn get_status_report(
    vault: &Vault,
    project_id: &ProjectId,
    rotation_reminder_days: u32,
) -> Result<Vec<StatusRow>, CoreError> {
    let rows = vault
        .environment_status(project_id)
        .map_err(CoreError::Db)?;

    let mut result = Vec::with_capacity(rows.len());
    for es in rows {
        let sync_status = derive_sync_status(es.last_modified_at, es.sealed_at);
        let stale_secrets = stale_secret_keys(vault, &es.id, rotation_reminder_days)?;
        result.push(StatusRow {
            name: es.name,
            secret_count: es.secret_count,
            last_modified_at: es.last_modified_at,
            sealed_at: es.sealed_at,
            sync_status,
            stale_secrets,
        });
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T019 — sealed_at None → NeverSealed
    #[test]
    fn derive_sync_status_never_sealed_when_no_marker() {
        assert_eq!(
            derive_sync_status(None, None),
            SyncStatus::NeverSealed,
            "no sealed_at must yield NeverSealed"
        );
        assert_eq!(
            derive_sync_status(Some(1_000), None),
            SyncStatus::NeverSealed,
            "secrets present but no sealed_at must yield NeverSealed"
        );
    }

    // T020 — last_modified_at > sealed_at → Modified
    #[test]
    fn derive_sync_status_modified_when_secret_newer_than_seal() {
        assert_eq!(
            derive_sync_status(Some(2_000), Some(1_000)),
            SyncStatus::Modified,
            "secret modified after seal must yield Modified"
        );
    }

    // T021 — last_modified_at == sealed_at → InSync
    #[test]
    fn derive_sync_status_in_sync_when_modified_equals_sealed() {
        assert_eq!(
            derive_sync_status(Some(1_000), Some(1_000)),
            SyncStatus::InSync,
            "secret modified at exactly the seal time must yield InSync"
        );
    }

    // T022 — last_modified_at < sealed_at → InSync
    #[test]
    fn derive_sync_status_in_sync_when_secret_older_than_seal() {
        assert_eq!(
            derive_sync_status(Some(500), Some(1_000)),
            SyncStatus::InSync,
            "secret modified before seal must yield InSync"
        );
    }

    // T023 — no secrets (last_modified_at None) + sealed_at Some → InSync
    #[test]
    fn derive_sync_status_in_sync_for_empty_env_with_marker() {
        assert_eq!(
            derive_sync_status(None, Some(1_000)),
            SyncStatus::InSync,
            "empty env with a sync marker must yield InSync"
        );
    }
}
