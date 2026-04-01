//! Core Logic layer — orchestrates secret CRUD, project context resolution,
//! and environment management.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`.
//! - MAY import from `crate::db` and `crate::crypto` only.

pub mod diff;
mod error;
mod manifest;
mod ops;
pub mod status;
pub mod sync;

pub use diff::{ChangeType, DiffEntry, DiffReport, compute_diff};
pub use error::CoreError;
pub use manifest::{Manifest, create_manifest, find_manifest};
pub use ops::{
    DEFAULT_ENV, delete_secret, get_env_secrets, get_secret, list_secret_keys,
    list_secrets_with_values, set_secret,
};
pub use status::{StatusRow, SyncStatus, derive_sync_status, get_status_report};
pub use sync::{
    SyncError, UnsealResult, check_envelope_passphrase, new_empty_artifact, read_artifact,
    seal_artifact, seal_env, unseal_artifact, unseal_env, write_artifact, write_artifact_atomic,
};
