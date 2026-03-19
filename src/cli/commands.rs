//! Command handler functions — see plan.md §4.2
//!
//! All functions are `pub(super)` — only callable from `cli::run()`.
//! Handlers receive already-opened resources (`&Vault`, `&[u8; 32]`,
//! `&ProjectId`) and must not manage the Vault lifecycle themselves.
//!
//! # Exception
//! [`cmd_init`] is the sole exception: it owns its own Vault connection
//! because init creates the project entry before any manifest exists.

use std::path::Path;

use crate::cli::error::CliError;
use crate::core::CoreError;
use crate::db::{ProjectId, Vault};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns the display name for an environment, substituting the Core default
/// when the CLI receives an empty string (meaning `-e` flag was absent).
///
/// The CLI must not hardcode `"development"` — `core::DEFAULT_ENV` is the
/// single source of truth.
fn display_env(env: &str) -> &str {
    if env.is_empty() {
        crate::core::DEFAULT_ENV
    } else {
        env
    }
}

// ---------------------------------------------------------------------------
// T016 — cmd_init
// ---------------------------------------------------------------------------

/// Initialises Envy in the current working directory.
///
/// Creates `envy.toml` and registers a new project entry in the vault.
/// This is the only handler that owns its own Vault connection.
///
/// # Errors
/// - [`CliError::AlreadyInitialised`] — `envy.toml` exists in the cwd.
/// - [`CliError::ParentProjectExists`] — `envy.toml` exists in a parent dir.
/// - [`CliError::VaultOpen`] — keyring, vault open, or DB write failed.
pub(super) fn cmd_init() -> Result<(), CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::VaultOpen(format!("cannot determine current directory: {e}")))?;

    // Step 1 — Check whether a manifest already exists (in cwd or any ancestor).
    match crate::core::find_manifest(&cwd) {
        Ok((_, found_dir)) if found_dir == cwd => {
            return Err(CliError::AlreadyInitialised);
        }
        Ok((_, found_dir)) => {
            return Err(CliError::ParentProjectExists(
                found_dir.display().to_string(),
            ));
        }
        Err(CoreError::ManifestNotFound) => {
            // No manifest anywhere above — safe to initialise.
        }
        Err(e) => {
            return Err(CliError::VaultOpen(e.to_string()));
        }
    }

    // Step 2 — Fetch (or generate) the global vault master key from the OS keyring.
    let master_key = crate::crypto::get_or_create_master_key()
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 3 — Ensure the vault directory exists, then open (or create) the vault.
    let vault_path = super::vault_path();
    if let Some(vault_dir) = vault_path.parent() {
        std::fs::create_dir_all(vault_dir)
            .map_err(|e| CliError::VaultOpen(format!("cannot create vault directory: {e}")))?;
    }
    let vault = Vault::open(&vault_path, master_key.as_ref())
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 4 — Register the project in the vault. Use the cwd name as the
    // human-readable project name; the DB generates the UUID primary key.
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");
    let project_id: ProjectId = vault
        .create_project(project_name)
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 5 — Write envy.toml with the DB-generated project UUID.
    crate::core::create_manifest(&cwd, project_id.as_str())
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    println!("✓ Initialised envy project {}.", project_id.as_str());
    Ok(())
}

// ---------------------------------------------------------------------------
// T017 — cmd_set
// ---------------------------------------------------------------------------

/// Stores or updates a secret in the vault.
///
/// The `assignment` splitting (`KEY=VALUE` → `key`, `value`) is performed by
/// `cli::run()` before dispatch; `cmd_set` receives the split parts.
/// Core's `set_secret` is responsible for key-name validation.
pub(super) fn cmd_set(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    key: &str,
    value: &str,
) -> Result<(), CoreError> {
    crate::core::set_secret(vault, master_key, project_id, env, key, value)?;
    println!("✓ Set {} in {}.", key, display_env(env));
    Ok(())
}

// ---------------------------------------------------------------------------
// T018 — cmd_get
// ---------------------------------------------------------------------------

/// Prints the decrypted value of a secret to stdout.
///
/// **stdout contract**: outputs exactly `{value}\n` — no labels, no leading
/// whitespace. Shell pipelines (`envy get KEY | xargs ...`) depend on this.
pub(super) fn cmd_get(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    key: &str,
) -> Result<(), CoreError> {
    let value = crate::core::get_secret(vault, master_key, project_id, env, key)?;
    // `println!` appends exactly one newline — satisfies the stdout contract.
    println!("{}", *value);
    Ok(())
}

// ---------------------------------------------------------------------------
// T019 — cmd_list
// ---------------------------------------------------------------------------

/// Lists all secret key names for the environment (never their values).
///
/// Keys are printed one per line in alphabetical order (sorted by Core).
/// If the environment has no secrets, an informational message is printed
/// to stderr (not stdout) so that scripts consuming stdout are unaffected.
pub(super) fn cmd_list(vault: &Vault, project_id: &ProjectId, env: &str) -> Result<(), CoreError> {
    let keys = crate::core::list_secret_keys(vault, project_id, env)?;
    if keys.is_empty() {
        eprintln!("(no secrets in {})", display_env(env));
    } else {
        for k in &keys {
            println!("{k}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// T020 — cmd_rm
// ---------------------------------------------------------------------------

/// Permanently deletes a secret from the vault.
pub(super) fn cmd_rm(
    vault: &Vault,
    project_id: &ProjectId,
    env: &str,
    key: &str,
) -> Result<(), CoreError> {
    crate::core::delete_secret(vault, project_id, env, key)?;
    println!("✓ Deleted {} from {}.", key, display_env(env));
    Ok(())
}

// ---------------------------------------------------------------------------
// T025 — cmd_run
// ---------------------------------------------------------------------------

/// Injects all secrets for the environment as env vars and runs a child process.
///
/// # Exit code contract (from `contracts/cli.md`)
/// - Returns the child's exit code exactly as received.
/// - Returns `1` when the child is killed by a signal (Unix: `status.code()` → `None`).
/// - Returns `127` when the binary cannot be spawned (conventional "not found" code).
///
/// Secrets are injected **in addition to** the inherited environment, not as a
/// replacement. The `Zeroizing<String>` values are zeroed when the map is dropped.
pub(super) fn cmd_run(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    command: &[String],
) -> i32 {
    // Decrypt all secrets for the target environment.
    let secrets = match crate::core::get_env_secrets(vault, master_key, project_id, env) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", crate::cli::error::format_core_error(&e));
            return crate::cli::error::core_exit_code(&e);
        }
    };

    // `command` is guaranteed non-empty by `#[arg(last = true, required = true)]`.
    let (bin, args) = command
        .split_first()
        .expect("clap guarantees at least one element after --");

    match std::process::Command::new(bin)
        .args(args)
        // Inject secrets on top of the inherited environment.
        .envs(secrets.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .status()
    {
        Ok(status) => {
            // `status.code()` returns None when the child was killed by a Unix signal.
            // Fall back to 1 (generic failure) — full signal forwarding is Phase 3 work.
            status.code().unwrap_or(1)
        }
        Err(e) => {
            eprintln!("error: failed to execute `{}`: {}", bin, e);
            127 // conventional POSIX "command not found" exit code
        }
    }
}

// ---------------------------------------------------------------------------
// T026 — cmd_migrate
// ---------------------------------------------------------------------------

/// Imports secrets from a legacy `.env` file into the vault.
///
/// # Line-parsing rules (from `contracts/cli.md`)
/// 1. Trim leading/trailing whitespace.
/// 2. Skip blank lines and `#`-prefixed comment lines silently.
/// 3. Split on the **first** `=` only:
///    - `Some((key, value))` → call `core::set_secret` (abort on error).
///    - `None` → emit a per-line warning to stderr, continue.
/// 4. After all lines: print a summary to stdout.
///
/// Migration is NOT atomic: if aborted mid-way, partial secrets remain.
/// Re-running is safe because `set_secret` uses upsert semantics.
pub(super) fn cmd_migrate(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    file: &Path,
) -> Result<(), CliError> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| CliError::FileNotFound(file.display().to_string(), e.to_string()))?;

    let mut imported = 0usize;
    let mut warnings = 0usize;

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Rule 2: skip blank lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Rule 3: split on first `=`.
        match trimmed.split_once('=') {
            Some((key, value)) => {
                crate::core::set_secret(vault, master_key, project_id, env, key.trim(), value)
                    .map_err(|e| CliError::VaultOpen(e.to_string()))?;
                imported += 1;
            }
            None => {
                eprintln!(
                    "warning: line {}: skipping malformed entry: {:?}",
                    line_no + 1,
                    trimmed
                );
                warnings += 1;
            }
        }
    }

    let warnings_suffix = if warnings > 0 {
        format!(" ({warnings} warning(s))")
    } else {
        String::new()
    };
    println!(
        "✓ Imported {} secret(s) into {}{}.",
        imported,
        display_env(env),
        warnings_suffix,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T008–T011, T022–T023 — Unit tests (written FIRST per TDD discipline)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // -----------------------------------------------------------------------
    // T008–T011 — assignment parsing (Phase 3)
    // -----------------------------------------------------------------------

    // T008
    #[test]
    fn parse_assignment_basic() {
        assert_eq!("KEY=VALUE".split_once('='), Some(("KEY", "VALUE")));
    }

    // T009
    #[test]
    fn parse_assignment_value_contains_equals() {
        // The first `=` is the separator; the rest of the value is preserved.
        assert_eq!("TOKEN=abc=def".split_once('='), Some(("TOKEN", "abc=def")));
    }

    // T010
    #[test]
    fn parse_assignment_no_equals() {
        // A token with no `=` must be detected as malformed (→ CliError::InvalidAssignment).
        assert_eq!("NOVALUE".split_once('='), None);
    }

    // T011
    #[test]
    fn parse_assignment_empty_key() {
        // "=VALUE" splits into ("", "VALUE") — Core's validate_key rejects the empty key.
        assert_eq!("=VALUE".split_once('='), Some(("", "VALUE")));
    }

    // -----------------------------------------------------------------------
    // T022–T023 — migrate line-parsing logic (Phase 4, written before impl)
    //
    // These tests exercise the exact parsing pattern used by cmd_migrate:
    //   trim → skip blank/comment → split_once('=') → import or warn
    // They are pure logic tests with no I/O, vault, or crypto involvement.
    // -----------------------------------------------------------------------

    /// Shared helper: runs the migrate line-parser over `input` and returns
    /// (valid_pairs, malformed_count) — mirroring cmd_migrate's inner loop.
    fn parse_env_lines<'a>(input: &'a str) -> (Vec<(&'a str, &'a str)>, usize) {
        let mut valid: Vec<(&str, &str)> = Vec::new();
        let mut malformed = 0usize;
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            match trimmed.split_once('=') {
                Some((k, v)) => valid.push((k, v)),
                None => malformed += 1,
            }
        }
        (valid, malformed)
    }

    // T022
    #[test]
    fn migrate_skips_comments_and_blanks() {
        let input = "KEY1=value1\n# this is a comment\n\nKEY2=value2\n";
        let (valid, malformed) = parse_env_lines(input);

        assert_eq!(valid.len(), 2, "must produce exactly 2 valid pairs");
        assert_eq!(malformed, 0, "must report 0 malformed lines");
        assert_eq!(valid[0], ("KEY1", "value1"));
        assert_eq!(valid[1], ("KEY2", "value2"));
    }

    // T023
    #[test]
    fn migrate_warns_on_malformed() {
        let input = "GOOD_KEY=good_value\nBADLINE\n";
        let (valid, malformed) = parse_env_lines(input);

        assert_eq!(valid.len(), 1, "must produce exactly 1 valid pair");
        assert_eq!(malformed, 1, "must detect exactly 1 malformed line");
        assert_eq!(valid[0], ("GOOD_KEY", "good_value"));
    }
}
