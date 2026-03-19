//! Secret upsert / get / list / delete operations.
//!
//! All functions are methods on [`Vault`] and are implemented here via an `impl`
//! block in this file. They are re-exported through `src/db/mod.rs`.
//!
//! # Layer rules
//! - This file MUST NOT import from `crate::cli`, `crate::core`, or `crate::crypto`.
//! - All functions return `Result<T, DbError>`.
//! - `.unwrap()` is prohibited; use `?` or `map_err`.
//!
//! # Security contract
//! - `value_encrypted` and `value_nonce` MUST NEVER be logged or printed anywhere
//!   in this layer. They are opaque byte blobs from the crypto layer.
//! - The DB layer is unaware of plaintext — it stores and returns raw bytes only.

use rusqlite::params;
use uuid::Uuid;

use super::{
    error::{map_rusqlite_error, not_found_or, DbError},
    EnvId, SecretId, Vault,
};

/// A secret record as stored in the vault.
///
/// `value_encrypted` and `value_nonce` are opaque byte blobs produced and
/// consumed exclusively by the crypto layer. The DB layer never inspects them.
#[derive(Debug, Clone)]
pub struct SecretRecord {
    /// Globally unique identifier (UUID v4, hyphenated).
    pub id: SecretId,
    /// The environment this secret belongs to.
    pub environment_id: EnvId,
    /// Secret key name (e.g., `DATABASE_URL`).
    pub key: String,
    /// AES-256-GCM ciphertext bytes.
    pub value_encrypted: Vec<u8>,
    /// 12-byte (96-bit) random nonce for this row's AES-256-GCM encryption.
    pub value_nonce: Vec<u8>,
    /// Creation time as Unix epoch (UTC, seconds).
    pub created_at: i64,
    /// Last-modification time as Unix epoch (UTC, seconds).
    pub updated_at: i64,
}

impl Vault {
    /// Inserts or replaces a secret for `(env_id, key)`.
    ///
    /// If a secret with the same `key` already exists in `env_id`, the old row
    /// is atomically replaced (its UUID changes — this is acceptable in Phase 1).
    ///
    /// # Caller contract
    /// - `value_nonce` MUST be exactly 12 bytes. Returns
    ///   `DbError::ConstraintViolation` immediately if not.
    /// - `value_encrypted` is an opaque blob — this layer does not interpret it.
    ///
    /// # Security
    /// `value_encrypted` and `value_nonce` MUST NOT be logged or printed here.
    pub fn upsert_secret(
        &self,
        env_id: &EnvId,
        key: &str,
        value_encrypted: &[u8],
        value_nonce: &[u8],
    ) -> Result<SecretId, DbError> {
        if value_nonce.len() != 12 {
            return Err(DbError::ConstraintViolation(
                "nonce must be exactly 12 bytes".into(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO secrets
                 (id, environment_id, key, value_encrypted, value_nonce,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'), strftime('%s','now'))",
                params![id, env_id.as_str(), key, value_encrypted, value_nonce],
            )
            .map_err(map_rusqlite_error)?;

        Ok(SecretId(id))
    }

    /// Returns the secret for `(env_id, key)`, or [`DbError::NotFound`] if none exists.
    pub fn get_secret(&self, env_id: &EnvId, key: &str) -> Result<SecretRecord, DbError> {
        self.conn
            .query_row(
                "SELECT id, environment_id, key, value_encrypted, value_nonce,
                        created_at, updated_at
                 FROM secrets
                 WHERE environment_id = ?1 AND key = ?2",
                params![env_id.as_str(), key],
                row_to_secret,
            )
            .map_err(not_found_or)
    }

    /// Returns all secrets for `env_id` ordered by `key ASC`.
    ///
    /// Returns an empty `Vec` if none exist.
    pub fn list_secrets(&self, env_id: &EnvId) -> Result<Vec<SecretRecord>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, environment_id, key, value_encrypted, value_nonce,
                        created_at, updated_at
                 FROM secrets
                 WHERE environment_id = ?1
                 ORDER BY key ASC",
            )
            .map_err(map_rusqlite_error)?;

        let rows = stmt
            .query_map(params![env_id.as_str()], row_to_secret)
            .map_err(map_rusqlite_error)?;

        rows.map(|r| r.map_err(map_rusqlite_error))
            .collect::<Result<Vec<SecretRecord>, DbError>>()
    }

    /// Deletes the secret identified by `(env_id, key)`.
    ///
    /// Returns [`DbError::NotFound`] if no matching secret exists.
    pub fn delete_secret(&self, env_id: &EnvId, key: &str) -> Result<(), DbError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM secrets WHERE environment_id = ?1 AND key = ?2",
                params![env_id.as_str(), key],
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

/// Maps a rusqlite `Row` to a [`SecretRecord`] struct.
fn row_to_secret(row: &rusqlite::Row<'_>) -> rusqlite::Result<SecretRecord> {
    Ok(SecretRecord {
        id: SecretId(row.get(0)?),
        environment_id: EnvId(row.get(1)?),
        key: row.get(2)?,
        value_encrypted: row.get(3)?,
        value_nonce: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}
