//! Core Logic layer — orchestrates secret CRUD, project context resolution,
//! and environment management.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`.
//! - MAY import from `crate::db` and `crate::crypto` only.

mod error;
mod manifest;
mod ops;
pub mod sync;

pub use error::CoreError;
pub use manifest::{Manifest, create_manifest, find_manifest};
pub use ops::{
    DEFAULT_ENV, delete_secret, get_env_secrets, get_secret, list_secret_keys,
    list_secrets_with_values, set_secret,
};
pub use sync::{
    SyncError, UnsealResult, read_artifact, seal_artifact, unseal_artifact, write_artifact,
};
