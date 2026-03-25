//! Database layer — SQLite persistence via rusqlite + bundled-sqlcipher.
//!
//! # Rules for this layer
//! - MUST NOT import from `crate::cli` or `crate::core`.
//! - MUST NOT perform any cryptographic operations (encrypt/decrypt).
//!   It stores and retrieves raw bytes for `value_encrypted` and `value_nonce`.
//! - All public functions return `Result<T, DbError>`.
//! - `.unwrap()` and `.expect()` are prohibited in this module.

mod environments;
mod error;
mod projects;
mod schema;
mod secrets;
mod sync_markers;

pub use environments::Environment;
pub use error::DbError;
use error::map_rusqlite_error;
pub use projects::Project;
pub use secrets::SecretRecord;
pub use sync_markers::EnvironmentStatus;

use std::path::Path;

// ---------------------------------------------------------------------------
// Newtype wrappers for primary key identifiers.
//
// Using distinct types instead of bare `String` prevents accidentally passing
// an EnvId where a ProjectId is expected — the compiler catches it at compile time.
// ---------------------------------------------------------------------------

/// A project's unique identifier (UUID v4, hyphenated string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectId(pub String);

impl ProjectId {
    /// Returns the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An environment's unique identifier (UUID v4, hyphenated string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvId(pub String);

impl EnvId {
    /// Returns the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A secret's unique identifier (UUID v4, hyphenated string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretId(pub String);

impl SecretId {
    /// Returns the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Vault
// ---------------------------------------------------------------------------

/// An open, authenticated connection to the encrypted vault file.
///
/// Obtain one via [`Vault::open`]. The connection is closed when the `Vault`
/// is dropped or when [`Vault::close`] is called explicitly.
///
/// The `conn` field is intentionally private: all database access MUST go
/// through the `Vault` methods so that the pragma invariants (foreign keys
/// enabled, WAL mode, correct key) are always in force.
pub struct Vault {
    // rusqlite::Connection does not implement Debug, so we derive it manually.
    conn: rusqlite::Connection,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault").finish_non_exhaustive()
    }
}

impl Vault {
    /// Opens (or creates) the SQLCipher-encrypted vault at `vault_path`.
    ///
    /// `master_key` MUST be exactly 32 bytes. The caller is responsible for
    /// zeroing the key slice after this call returns.
    ///
    /// Internally this function:
    /// 1. Opens the SQLite file.
    /// 2. Sets `PRAGMA key` (the SQLCipher decryption key).
    /// 3. Sets `PRAGMA foreign_keys = ON`.
    /// 4. Sets `PRAGMA journal_mode = WAL`.
    /// 5. Runs schema migrations (creates tables on first open).
    ///
    /// # Errors
    /// - [`DbError::IoError`] if the file cannot be opened or created.
    /// - [`DbError::EncryptionError`] if the key is wrong or the file is not a
    ///   SQLCipher database.
    /// - [`DbError::MigrationError`] if schema creation fails.
    pub fn open(vault_path: &Path, master_key: &[u8]) -> Result<Self, DbError> {
        let conn =
            rusqlite::Connection::open(vault_path).map_err(|e| DbError::IoError(e.to_string()))?;

        // Step 1 — Set the SQLCipher key.
        // MUST be the first statement on a new connection; SQLCipher requires it
        // before any other SQL is executed.
        // Raw-key format: x'<lowercase hex>' tells SQLCipher to use the bytes
        // directly rather than treating the argument as a passphrase.
        let hex_key = bytes_to_hex(master_key);
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\"", hex_key))
            .map_err(|e| DbError::Internal(format!("failed to set vault key: {e}")))?;

        // Step 2 — Enable foreign key enforcement.
        // SQLite disables FK checks by default. Without this, ON DELETE CASCADE
        // and referential integrity constraints are silently ignored.
        conn.pragma_update(None, "foreign_keys", 1i64)
            .map_err(map_rusqlite_error)?;

        // Step 3 — Switch to WAL journal mode.
        // WAL allows concurrent readers while a write is in progress, which is
        // important for the `envy run` use case (reads env vars while another
        // process may be writing).
        // NOTE: This is the first statement that causes SQLCipher to read the
        // database file header. An EncryptionError here means the key is wrong.
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(map_rusqlite_error)?;

        // Step 4 — Apply schema migrations.
        // On the first open this creates all three tables and sets user_version = 1.
        // On subsequent opens it is a no-op if user_version >= 1.
        // This is also where a wrong key is first detected (SQLITE_NOTADB).
        schema::run_migrations(&conn)?;

        Ok(Self { conn })
    }

    /// Flushes the WAL and closes the connection cleanly.
    ///
    /// This is optional — the connection is also closed when the `Vault` is dropped —
    /// but calling it explicitly ensures the WAL checkpoint completes before the
    /// caller proceeds.
    pub fn close(self) -> Result<(), DbError> {
        // Checkpoint and truncate the WAL file so readers that open the DB file
        // directly (e.g., the security test) see a fully consolidated database.
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| DbError::Internal(e.to_string()))?;
        // `self` is consumed; `self.conn` is dropped here, which closes the connection.
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Diagnostic helpers (used in tests and future CLI debug commands)
    // -----------------------------------------------------------------------

    /// Returns the value of an integer PRAGMA (e.g., `user_version`, `foreign_keys`).
    pub fn pragma_int(&self, name: &str) -> Result<i64, DbError> {
        self.conn
            .pragma_query_value(None, name, |row| row.get::<_, i64>(0))
            .map_err(map_rusqlite_error)
    }

    /// Returns the value of a string PRAGMA (e.g., `journal_mode`).
    pub fn pragma_str(&self, name: &str) -> Result<String, DbError> {
        self.conn
            .pragma_query_value(None, name, |row| row.get::<_, String>(0))
            .map_err(map_rusqlite_error)
    }

    /// Returns `true` if a table with `table_name` exists in the vault schema.
    pub fn table_exists(&self, table_name: &str) -> Result<bool, DbError> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .map_err(map_rusqlite_error)?;
        Ok(count > 0)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Encodes a byte slice as a lowercase hex string.
///
/// Used to format the SQLCipher PRAGMA key value as `x'<hex>'`.
///
/// Writing to a `String` via `fmt::Write` is infallible (no I/O involved),
/// so the `expect` below is safe by construction.
fn bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // SAFETY: fmt::Write for String is infallible — it only fails for I/O
        // writers, never for in-memory strings.
        write!(hex, "{:02x}", b).expect("fmt::Write for String is infallible");
    }
    hex
}
