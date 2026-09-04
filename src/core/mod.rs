//! Core Logic layer — orchestrates secret CRUD, project context resolution,
//! and environment management.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`.
//! - MAY import from `crate::db` and `crate::crypto` only.

pub mod audit;
pub mod diff;
pub mod discover;
mod error;
mod manifest;
mod ops;
pub mod scan;
pub mod status;
pub mod sync;

pub use audit::{AuditAction, AuditEntry, list_audit, record as record_audit};
pub use diff::{ChangeType, DiffEntry, DiffReport, compute_diff};
pub use discover::{DiscoveredProject, discover_projects};
pub use error::CoreError;
pub use manifest::{Manifest, create_manifest, find_manifest};
pub use ops::{
    DEFAULT_ENV, EnvironmentSummary, ProjectSummary, SecretValueSummary, delete_project,
    delete_secret, get_env_secrets, get_secret, list_environments, list_projects, list_secret_keys,
    list_secrets_with_metadata, list_secrets_with_values, project_deletion_counts, set_secret,
};
pub use scan::{ScanMatch, scan_for_leaks};
pub use status::{StatusRow, SyncStatus, derive_sync_status, get_status_report};
pub use sync::{
    SyncError, UnsealResult, check_envelope_passphrase, mark_env_sealed, new_empty_artifact,
    read_artifact, rotate_env, seal_artifact, seal_env, seal_env_unmarked, unseal_artifact,
    unseal_env, write_artifact, write_artifact_atomic,
};
