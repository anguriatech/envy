/// All errors that the database layer can return to the Core layer.
///
/// The Core layer MUST match on these variants and never inspect raw rusqlite errors
/// directly — the database layer is the only place that knows about rusqlite internals.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A queried record does not exist.
    #[error("record not found")]
    NotFound,

    /// A unique constraint was violated (e.g., duplicate environment name for a project).
    #[error("record already exists")]
    AlreadyExists,

    /// A constraint other than uniqueness was violated (e.g., CHECK, FK without cascade).
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    /// The vault file could not be opened or created (filesystem-level error).
    #[error("I/O error: {0}")]
    IoError(String),

    /// The master key is wrong, or the vault file is corrupted / not a SQLCipher database.
    #[error("wrong master key or corrupted vault")]
    EncryptionError,

    /// The schema migration step failed.
    #[error("schema migration failed: {0}")]
    MigrationError(String),

    /// An unexpected internal rusqlite error that does not map to a specific variant.
    #[error("internal database error: {0}")]
    Internal(String),
}

/// Maps a rusqlite `QueryReturnedNoRows` error to `DbError::NotFound`, and any other
/// error to its appropriate `DbError` variant via [`map_rusqlite_error`].
///
/// Use this as the `.map_err` closure on any `query_row` call where a missing row
/// should be surfaced as `DbError::NotFound`:
///
/// ```ignore
/// conn.query_row("SELECT ...", params![id], row_mapper)
///     .map_err(not_found_or)
/// ```
pub(super) fn not_found_or(e: rusqlite::Error) -> DbError {
    if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
        DbError::NotFound
    } else {
        map_rusqlite_error(e)
    }
}

/// Returns true when a rusqlite error indicates the database file is not valid SQLCipher
/// (i.e., the wrong master key was supplied, or the file is not a database at all).
/// SQLite error code 26 = SQLITE_NOTADB.
pub(super) fn is_encryption_error(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(err, _) if err.extended_code == 26)
}

/// Maps a rusqlite error to the appropriate DbError variant, detecting constraint
/// violations and encryption errors before falling back to Internal.
pub(super) fn map_rusqlite_error(e: rusqlite::Error) -> DbError {
    if is_encryption_error(&e) {
        return DbError::EncryptionError;
    }
    if let rusqlite::Error::SqliteFailure(ref err, ref msg) = e {
        // SQLITE_CONSTRAINT_UNIQUE = 2067, SQLITE_CONSTRAINT_PRIMARYKEY = 1555
        if err.extended_code == 2067 || err.extended_code == 1555 {
            return DbError::AlreadyExists;
        }
        // Other constraint violations (FK, CHECK, NOT NULL, etc.)
        if err.code == rusqlite::ErrorCode::ConstraintViolation {
            return DbError::ConstraintViolation(msg.clone().unwrap_or_else(|| e.to_string()));
        }
    }
    DbError::Internal(e.to_string())
}
