//! Environment CRUD operations.
//!
//! All functions are methods on [`Vault`] and are implemented here via an `impl`
//! block in this file. They are re-exported through `src/db/mod.rs`.
//!
//! # Layer rules
//! - This file MUST NOT import from `crate::cli`, `crate::core`, or `crate::crypto`.
//! - All functions return `Result<T, DbError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.
//!
//! # Name normalization contract
//! Environment names MUST be lowercased by the caller before passing to
//! `create_environment`. The `CHECK(name = lower(name))` constraint in the schema
//! enforces this at the DB level as a second line of defense.

use rusqlite::params;
use uuid::Uuid;

use super::{
    EnvId, ProjectId, Vault,
    error::{DbError, map_rusqlite_error, not_found_or},
};

/// An environment record as stored in the vault.
#[derive(Debug, Clone)]
pub struct Environment {
    /// Globally unique identifier (UUID v4, hyphenated).
    pub id: EnvId,
    /// The project this environment belongs to.
    pub project_id: ProjectId,
    /// Lowercase environment label (e.g., `development`, `production`).
    pub name: String,
    /// Creation time as Unix epoch (UTC, seconds).
    pub created_at: i64,
    /// Last-modification time as Unix epoch (UTC, seconds).
    pub updated_at: i64,
}

impl Vault {
    /// Creates a new environment scoped to `project_id` and returns its [`EnvId`].
    ///
    /// # Caller contract
    /// `name` MUST be lowercased before calling this function. The schema's
    /// `CHECK(name = lower(name))` constraint will reject uppercase names with a
    /// `DbError::ConstraintViolation`.
    ///
    /// # Errors
    /// - [`DbError::AlreadyExists`] if an environment with the same name already exists
    ///   within `project_id`.
    /// - [`DbError::ConstraintViolation`] if `project_id` does not exist (FK violation)
    ///   or if `name` contains uppercase characters.
    pub fn create_environment(&self, project_id: &ProjectId, name: &str) -> Result<EnvId, DbError> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO environments (id, project_id, name) VALUES (?1, ?2, ?3)",
                params![id, project_id.as_str(), name],
            )
            .map_err(map_rusqlite_error)?;
        Ok(EnvId(id))
    }

    /// Returns the environment with the given `id`, or [`DbError::NotFound`] if it
    /// does not exist.
    pub fn get_environment(&self, id: &EnvId) -> Result<Environment, DbError> {
        self.conn
            .query_row(
                "SELECT id, project_id, name, created_at, updated_at
                 FROM environments
                 WHERE id = ?1",
                params![id.as_str()],
                row_to_environment,
            )
            .map_err(not_found_or)
    }

    /// Returns the environment whose `name` matches exactly within `project_id`, or
    /// [`DbError::NotFound`] if none exists.
    pub fn get_environment_by_name(
        &self,
        project_id: &ProjectId,
        name: &str,
    ) -> Result<Environment, DbError> {
        self.conn
            .query_row(
                "SELECT id, project_id, name, created_at, updated_at
                 FROM environments
                 WHERE project_id = ?1 AND name = ?2",
                params![project_id.as_str(), name],
                row_to_environment,
            )
            .map_err(not_found_or)
    }

    /// Returns all environments for `project_id` ordered by `name ASC`.
    ///
    /// Returns an empty `Vec` if none exist.
    pub fn list_environments(&self, project_id: &ProjectId) -> Result<Vec<Environment>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, project_id, name, created_at, updated_at
                 FROM environments
                 WHERE project_id = ?1
                 ORDER BY name ASC",
            )
            .map_err(map_rusqlite_error)?;

        let rows = stmt
            .query_map(params![project_id.as_str()], row_to_environment)
            .map_err(map_rusqlite_error)?;

        rows.map(|r| r.map_err(map_rusqlite_error))
            .collect::<Result<Vec<Environment>, DbError>>()
    }

    /// Deletes the environment with the given `id`.
    ///
    /// `ON DELETE CASCADE` automatically removes all secrets belonging to this
    /// environment.
    ///
    /// Returns [`DbError::NotFound`] if no environment with that id exists.
    pub fn delete_environment(&self, id: &EnvId) -> Result<(), DbError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM environments WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(map_rusqlite_error)?;

        if changed == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Maps a rusqlite `Row` to an [`Environment`] struct.
fn row_to_environment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Environment> {
    Ok(Environment {
        id: EnvId(row.get(0)?),
        project_id: ProjectId(row.get(1)?),
        name: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}
