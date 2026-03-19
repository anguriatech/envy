//! CLI-specific errors and exit-code mapping — see contracts/cli.md
//!
//! Two error surfaces are covered:
//! - [`CliError`]: errors that originate in the CLI layer itself (bad argument
//!   format, file not found, already-initialised conflict, etc.)
//! - [`core_exit_code`] / [`cli_exit_code`]: exit-code table used by `run()`
//!   to map any error to a POSIX exit code before calling `process::exit`.

use crate::core::CoreError;
use crate::db::DbError;

// ---------------------------------------------------------------------------
// T005 — CliError enum
// ---------------------------------------------------------------------------

/// Errors that originate in the CLI layer and do not come from Core.
///
/// All variants produce a human-readable `Display` message via `thiserror`.
/// The message is prefixed with `"error: "` by [`format_cli_error`] before
/// being printed to stderr.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// `set` argument lacked a `=` separator (e.g. `envy set NOVALUE`).
    #[error("invalid assignment \"{0}\": expected KEY=VALUE format")]
    InvalidAssignment(String),

    /// `migrate` target file could not be opened or read.
    #[error("cannot read file \"{0}\": {1}")]
    FileNotFound(String, String),

    /// `init` was run in a directory that already contains `envy.toml`.
    #[error("already initialised: envy.toml exists in this directory")]
    AlreadyInitialised,

    /// `init` was run inside a directory tree that already has a parent project.
    #[error("parent project detected: \"{0}\" already contains envy.toml")]
    ParentProjectExists(String),

    /// The vault file was opened but the project UUID was not found inside it.
    #[error("project not found in vault \u{2014} was the vault file moved?")]
    ProjectNotInVault,

    /// `Vault::open` failed (wrong key, corrupted file, or permission error).
    #[error("could not open vault: {0}")]
    VaultOpen(String),
}

// ---------------------------------------------------------------------------
// T006 — Error formatting helpers
// ---------------------------------------------------------------------------

/// Formats a [`CoreError`] as a user-readable terminal message.
///
/// Always prefixes with `"error: "` so all error output has a consistent style.
pub fn format_core_error(e: &CoreError) -> String {
    format!("error: {e}")
}

/// Formats a [`CliError`] as a user-readable terminal message.
///
/// Always prefixes with `"error: "` so all error output has a consistent style.
pub fn format_cli_error(e: &CliError) -> String {
    format!("error: {e}")
}

// ---------------------------------------------------------------------------
// T007 — Exit-code mappers
// ---------------------------------------------------------------------------

/// Maps a [`CoreError`] to a POSIX exit code.
///
/// Exit-code table (from `contracts/cli.md`):
///
/// | Code | Meaning |
/// |------|---------|
/// | 1    | Not found (manifest, secret) or I/O error on manifest |
/// | 2    | Invalid input (key name) |
/// | 4    | Vault / crypto failure |
pub fn core_exit_code(e: &CoreError) -> i32 {
    match e {
        CoreError::ManifestNotFound => 1,
        CoreError::ManifestInvalid(_) => 1,
        CoreError::ManifestIo(_) => 1,
        CoreError::InvalidSecretKey(_) => 2,
        // DbError::NotFound is a "not found" condition — exit 1.
        // All other Db errors (corruption, constraint) are vault failures — exit 4.
        CoreError::Db(DbError::NotFound) => 1,
        CoreError::Db(_) => 4,
        CoreError::Crypto(_) => 4,
    }
}

/// Maps a [`CliError`] to a POSIX exit code.
///
/// Exit-code table (from `contracts/cli.md`):
///
/// | Code | Meaning |
/// |------|---------|
/// | 1    | File not found |
/// | 2    | Invalid input (bad assignment format) |
/// | 3    | Initialisation conflict |
/// | 4    | Vault failure |
pub fn cli_exit_code(e: &CliError) -> i32 {
    match e {
        CliError::InvalidAssignment(_) => 2,
        CliError::FileNotFound(_, _) => 1,
        CliError::AlreadyInitialised => 3,
        CliError::ParentProjectExists(_) => 3,
        CliError::ProjectNotInVault => 4,
        CliError::VaultOpen(_) => 4,
    }
}

// ---------------------------------------------------------------------------
// T012–T014 — Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T012
    #[test]
    fn format_manifest_not_found() {
        let msg = format_core_error(&crate::core::CoreError::ManifestNotFound);
        assert!(
            msg.starts_with("error: "),
            "message must start with 'error: ', got: {msg:?}"
        );
        assert!(
            msg.contains("envy init"),
            "message must mention `envy init` to guide the user, got: {msg:?}"
        );
    }

    // T013
    #[test]
    fn exit_code_not_found() {
        assert_eq!(
            core_exit_code(&crate::core::CoreError::ManifestNotFound),
            1,
            "ManifestNotFound must map to exit code 1"
        );
        assert_eq!(
            core_exit_code(&crate::core::CoreError::Db(crate::db::DbError::NotFound)),
            1,
            "Db(NotFound) must map to exit code 1"
        );
    }

    // T014
    #[test]
    fn exit_code_invalid_key() {
        assert_eq!(
            core_exit_code(&crate::core::CoreError::InvalidSecretKey(String::new())),
            2,
            "InvalidSecretKey must map to exit code 2"
        );
    }
}
