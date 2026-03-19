//! Project CRUD operations.
//!
//! All functions are methods on [`Vault`] and are implemented here via an `impl`
//! block in this file. They are re-exported through `src/db/mod.rs`.
//!
//! # Layer rules
//! - This file MUST NOT import from `crate::cli`, `crate::core`, or `crate::crypto`.
//! - All functions return `Result<T, DbError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.

use rusqlite::params;
use uuid::Uuid;

use super::{
    error::{map_rusqlite_error, not_found_or, DbError},
    ProjectId, Vault,
};

/// A project record as stored in the vault.
#[derive(Debug, Clone)]
pub struct Project {
    /// Globally unique identifier (UUID v4, hyphenated).
    pub id: ProjectId,
    /// Human-readable project name.
    pub name: String,
    /// Creation time as Unix epoch (UTC, seconds).
    pub created_at: i64,
    /// Last-modification time as Unix epoch (UTC, seconds).
    pub updated_at: i64,
}

impl Vault {
    /// Creates a new project record and returns its generated [`ProjectId`].
    ///
    /// A new UUID v4 is generated for every call. Project names are not required
    /// to be unique — deduplication by name is the Core layer's responsibility.
    pub fn create_project(&self, name: &str) -> Result<ProjectId, DbError> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO projects (id, name) VALUES (?1, ?2)",
                params![id, name],
            )
            .map_err(map_rusqlite_error)?;
        Ok(ProjectId(id))
    }

    /// Returns the project with the given `id`, or [`DbError::NotFound`] if it
    /// does not exist.
    pub fn get_project(&self, id: &ProjectId) -> Result<Project, DbError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at
                 FROM projects
                 WHERE id = ?1",
                params![id.as_str()],
                row_to_project,
            )
            .map_err(not_found_or)
    }

    /// Returns the first project whose `name` matches exactly, or
    /// [`DbError::NotFound`] if none exists.
    ///
    /// Names are not unique — if multiple projects share a name, the one with
    /// the earliest `created_at` is returned (ORDER BY created_at ASC LIMIT 1).
    pub fn get_project_by_name(&self, name: &str) -> Result<Project, DbError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at
                 FROM projects
                 WHERE name = ?1
                 ORDER BY created_at ASC
                 LIMIT 1",
                params![name],
                row_to_project,
            )
            .map_err(not_found_or)
    }

    /// Returns all projects ordered by `created_at ASC`.
    ///
    /// Returns an empty `Vec` if no projects exist.
    pub fn list_projects(&self) -> Result<Vec<Project>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, created_at, updated_at
                 FROM projects
                 ORDER BY created_at ASC",
            )
            .map_err(map_rusqlite_error)?;

        let rows = stmt
            .query_map([], row_to_project)
            .map_err(map_rusqlite_error)?;

        rows.map(|r| r.map_err(map_rusqlite_error))
            .collect::<Result<Vec<Project>, DbError>>()
    }

    /// Deletes the project with the given `id`.
    ///
    /// `ON DELETE CASCADE` automatically removes all environments and secrets
    /// belonging to this project.
    ///
    /// Returns [`DbError::NotFound`] if no project with that id exists.
    pub fn delete_project(&self, id: &ProjectId) -> Result<(), DbError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM projects WHERE id = ?1",
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

/// Maps a rusqlite `Row` to a [`Project`] struct.
///
/// Used as the row-mapping closure in every SELECT query in this module.
fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: ProjectId(row.get(0)?),
        name: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}
