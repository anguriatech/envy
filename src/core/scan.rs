//! Vault-leak scanner (`envy scan`).
//!
//! Walks the project's working tree looking for EXACT occurrences of secret
//! values that are **already stored in the vault** — this is deliberately
//! not a generic secrets-pattern scanner (no regex heuristics for "looks
//! like an AWS key"). Matching against known values gives near-zero false
//! positives: a hit means a value you are already managing with `envy` has
//! also been pasted in plaintext somewhere in the repo (the classic "copied
//! the .env value into a script and forgot" mistake).
//!
//! # Layer rules
//! - MUST NOT import from `crate::cli`.
//! - MAY import from `crate::db` and `crate::crypto`.
//!
//! # Security contract
//! - Matched values are never printed unless the caller explicitly asks for
//!   `reveal` — mirrors the `envy diff --reveal` contract.
//! - Every plaintext secret value is held in a [`zeroize::Zeroizing`] wrapper
//!   for as long as it's needed and no longer.

use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::db::{ProjectId, Vault};

use super::CoreError;

/// Secret values shorter than this are skipped. Matching a 2-3 character
/// value against arbitrary file content produces overwhelming noise (every
/// short common substring in the repo) with no forensic value.
const MIN_MATCH_LEN: usize = 6;

/// Files larger than this are skipped entirely, bounding worst-case scan
/// time when a large binary asset happens to be checked into the repo.
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB

/// Number of leading bytes inspected for the binary-file heuristic.
const BINARY_SNIFF_BYTES: usize = 8000;

/// One occurrence of a known vault secret found in plaintext in the working tree.
#[derive(Debug, Clone)]
pub struct ScanMatch {
    /// The secret's key name (e.g. `DATABASE_URL`).
    pub key: String,
    /// The environment the secret belongs to.
    pub environment: String,
    /// Path (relative to the scan root) where the match was found.
    pub path: PathBuf,
    /// 1-indexed line number within `path`.
    pub line: usize,
    /// The matched plaintext value — only populated when the caller passes `reveal: true`.
    pub value: Option<Zeroizing<String>>,
}

/// Scans `root` for plaintext occurrences of every secret in `env_name`
/// (or every environment in the vault when `env_name` is `None`).
///
/// Respects `.gitignore` / `.git/info/exclude` (via the `ignore` crate) so
/// `node_modules`, build output, and other already-excluded paths are not
/// scanned. Dotfiles ARE scanned — `.env` is the single most common leak
/// vector this feature targets. The `.git` directory itself and `envy.enc`
/// (ciphertext) are always skipped.
///
/// # Errors
/// - [`CoreError::Db`] if `env_name` does not exist, or another environment
///   lookup fails.
/// - [`CoreError::Crypto`] if any secret fails to decrypt.
pub fn scan_for_leaks(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: Option<&str>,
    root: &Path,
    reveal: bool,
) -> Result<Vec<ScanMatch>, CoreError> {
    let env_names: Vec<String> = match env_name {
        Some(n) => vec![n.to_lowercase()],
        None => vault
            .list_environments(project_id)?
            .into_iter()
            .map(|e| e.name)
            .collect(),
    };

    // (key, environment, value) needles to search for — collected once so
    // every file is only read and scanned a single time.
    let mut needles: Vec<(String, String, Zeroizing<String>)> = Vec::new();
    for name in &env_names {
        let pairs = super::list_secrets_with_values(vault, master_key, project_id, name)?;
        for (key, value) in pairs {
            if value.len() >= MIN_MATCH_LEN {
                needles.push((key, name.clone(), Zeroizing::new(value)));
            }
        }
    }

    let mut matches = Vec::new();
    if needles.is_empty() {
        return Ok(matches);
    }

    for path in walk_project_files(root) {
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.len() == 0 || metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if is_probably_binary(&bytes) {
            continue;
        }

        let text = String::from_utf8_lossy(&bytes);
        let display_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        for (line_no, line) in text.lines().enumerate() {
            for (key, env, needle) in &needles {
                if line.contains(needle.as_str()) {
                    matches.push(ScanMatch {
                        key: key.clone(),
                        environment: env.clone(),
                        path: display_path.clone(),
                        line: line_no + 1,
                        value: if reveal {
                            Some(Zeroizing::new(needle.to_string()))
                        } else {
                            None
                        },
                    });
                }
            }
        }
    }

    Ok(matches)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns every regular file under `root`, honouring `.gitignore` and
/// friends, but WITHOUT skipping hidden/dotfiles (see module docs).
fn walk_project_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // `.gitignore` must be honoured even when `root` isn't (yet) inside
        // an actual `.git` repository — envy projects are frequently
        // scanned before the first `git init`.
        .require_git(false)
        .build();

    for result in walker {
        let Ok(entry) = result else { continue };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        if path.file_name().is_some_and(|n| n == "envy.enc") {
            // Ciphertext — scanning it can never find a plaintext leak.
            continue;
        }
        out.push(path.to_path_buf());
    }
    out
}

/// Coarse binary-file heuristic: a NUL byte in the first
/// [`BINARY_SNIFF_BYTES`] bytes almost never occurs in genuine text.
///
/// Used to skip files where a byte-level substring match would either never
/// fire (compressed/encoded binaries) or produce unreadable output.
fn is_probably_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_BYTES).any(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0xABu8; 32];

    fn open_test_vault() -> (tempfile::TempDir, Vault, ProjectId) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("vault.db");
        let vault = Vault::open(&path, &TEST_KEY).expect("vault open");
        let pid = vault
            .create_project("test-project")
            .expect("create project");
        (tmp, vault, pid)
    }

    #[test]
    fn finds_leaked_secret_in_plaintext_file() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "API_KEY",
            "sk_live_super_secret_value",
        )
        .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(
            scan_root.path().join("leaked.txt"),
            "const key = 'sk_live_super_secret_value';\n",
        )
        .expect("write leaked file");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), false)
            .expect("scan must succeed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].key, "API_KEY");
        assert_eq!(matches[0].line, 1);
        assert!(
            matches[0].value.is_none(),
            "value must be masked by default"
        );
    }

    #[test]
    fn reveal_true_populates_value() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "KEY",
            "leaked-value-1",
        )
        .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(scan_root.path().join("f.txt"), "leaked-value-1\n").expect("write");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), true)
            .expect("scan must succeed");
        assert_eq!(
            matches[0].value.as_deref().map(|v| v.as_str()),
            Some("leaked-value-1")
        );
    }

    #[test]
    fn clean_repo_returns_no_matches() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "KEY",
            "totally-secret",
        )
        .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(scan_root.path().join("f.txt"), "nothing sensitive here\n").expect("write");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), false)
            .expect("scan must succeed");
        assert!(matches.is_empty());
    }

    #[test]
    fn short_secret_values_are_skipped() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(&vault, &TEST_KEY, &pid, "development", "SHORT", "abc")
            .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(scan_root.path().join("f.txt"), "abc\n").expect("write");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), false)
            .expect("scan must succeed");
        assert!(
            matches.is_empty(),
            "values shorter than MIN_MATCH_LEN must never be searched for"
        );
    }

    #[test]
    fn respects_gitignore() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "KEY",
            "ignored-secret",
        )
        .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(scan_root.path().join(".gitignore"), "ignored-dir/\n").expect("write");
        std::fs::create_dir(scan_root.path().join("ignored-dir")).expect("mkdir");
        std::fs::write(
            scan_root.path().join("ignored-dir").join("f.txt"),
            "ignored-secret\n",
        )
        .expect("write");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), false)
            .expect("scan must succeed");
        assert!(matches.is_empty(), "gitignored paths must not be scanned");
    }

    #[test]
    fn scans_dotfiles_like_env() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "KEY",
            "dotfile-secret",
        )
        .expect("set_secret");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(scan_root.path().join(".env"), "KEY=dotfile-secret\n").expect("write");

        let matches = scan_for_leaks(&vault, &TEST_KEY, &pid, None, scan_root.path(), false)
            .expect("scan must succeed");
        assert_eq!(
            matches.len(),
            1,
            "dotfiles like .env must be scanned, not skipped as 'hidden'"
        );
    }

    #[test]
    fn env_filter_restricts_which_secrets_are_searched() {
        let (_tmp, vault, pid) = open_test_vault();
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "DEV",
            "dev-secret-value",
        )
        .expect("set dev");
        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "production",
            "PROD",
            "prod-secret-value",
        )
        .expect("set prod");

        let scan_root = tempfile::tempdir().expect("scan root");
        std::fs::write(
            scan_root.path().join("f.txt"),
            "dev-secret-value\nprod-secret-value\n",
        )
        .expect("write");

        let matches = scan_for_leaks(
            &vault,
            &TEST_KEY,
            &pid,
            Some("production"),
            scan_root.path(),
            false,
        )
        .expect("scan must succeed");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].key, "PROD");
    }
}
