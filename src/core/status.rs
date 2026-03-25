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

use crate::db::{EnvironmentStatus, ProjectId, Vault};

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

/// Returns a [`StatusRow`] for every environment in `project_id`, ordered
/// alphabetically by environment name.
///
/// Calls [`Vault::environment_status`] in a single DB round-trip, then maps
/// each row through [`derive_sync_status`].
///
/// Returns an empty `Vec` if the project has no environments.
///
/// # Errors
/// - [`CoreError::Database`] for unexpected SQLite errors.
pub fn get_status_report(
    vault: &Vault,
    project_id: &ProjectId,
) -> Result<Vec<StatusRow>, CoreError> {
    let rows = vault
        .environment_status(project_id)
        .map_err(CoreError::Db)?;

    Ok(rows
        .into_iter()
        .map(|es: EnvironmentStatus| {
            let sync_status = derive_sync_status(es.last_modified_at, es.sealed_at);
            StatusRow {
                name: es.name,
                secret_count: es.secret_count,
                last_modified_at: es.last_modified_at,
                sealed_at: es.sealed_at,
                sync_status,
            }
        })
        .collect())
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
