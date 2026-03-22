//! GitOps sync artifact orchestration — seal, unseal, read, and write `envy.enc`.
//!
//! This module orchestrates the full artifact lifecycle: reading secrets from the
//! vault, sealing them into an `envy.enc` artifact via the crypto layer, and
//! unsealing an artifact back into a decrypted map for the CLI layer to write.
//!
//! # Layer rules (Constitution Principle IV)
//! - MAY import from `crate::crypto` and `crate::db` only.
//! - MUST NOT import from `crate::cli`.

use std::collections::BTreeMap;
use std::path::Path;

use zeroize::Zeroizing;

use crate::crypto::artifact::{
    ARTIFACT_VERSION, ArtifactError, ArtifactPayload, SyncArtifact, seal_envelope, unseal_envelope,
};
use crate::db::{ProjectId, Vault};

// ---------------------------------------------------------------------------
// T025 — SyncError
// ---------------------------------------------------------------------------

/// Errors at the sync artifact orchestration layer.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// A low-level artifact cryptography error.
    #[error(transparent)]
    Artifact(#[from] ArtifactError),

    /// `envy.enc` does not exist at the expected path.
    #[error("envy.enc not found at {0}")]
    FileNotFound(String),

    /// A file read or write operation failed.
    #[error("failed to read/write envy.enc: {0}")]
    Io(String),

    /// The artifact uses an unsupported schema version.
    #[error("envy.enc has unsupported schema version {0}")]
    UnsupportedVersion(u32),

    /// All environments were skipped; no secrets were imported.
    ///
    /// Not returned by `unseal_artifact` itself — reserved for the CLI layer to
    /// surface when `UnsealResult.imported` is empty.
    #[error("no environments could be decrypted \u{2014} check your passphrase")]
    NothingImported,

    /// A vault read failed while sealing.
    #[error("vault error: {0}")]
    VaultError(String),
}

// ---------------------------------------------------------------------------
// T026 — UnsealResult
// ---------------------------------------------------------------------------

/// The output of a successful [`unseal_artifact`] call.
///
/// Contains all environments that were successfully decrypted, plus a list of
/// environments that could not be decrypted (wrong passphrase or malformed
/// envelope). Callers MUST surface `skipped` as an informational warning.
///
/// # Memory safety
/// All values in `imported` are [`Zeroizing<String>`] — backing memory is
/// zeroed when the `UnsealResult` is dropped (Constitution Principle I).
pub struct UnsealResult {
    /// Successfully decrypted environments: env name → (secret key → plaintext value).
    pub imported: BTreeMap<String, BTreeMap<String, Zeroizing<String>>>,
    /// Environments that could not be decrypted (added to skipped, never hard-errors).
    pub skipped: Vec<String>,
}

// ---------------------------------------------------------------------------
// T027 — seal_artifact
// ---------------------------------------------------------------------------

/// Reads all secrets for `envs` from the vault and seals them into a [`SyncArtifact`].
///
/// Each environment is sealed independently with `passphrase`. Pass
/// `envs: None` to include every environment that exists in the vault; pass
/// `envs: Some(&["development", "staging"])` to seal only those names.
///
/// # Errors
/// - [`SyncError::Artifact(ArtifactError::WeakPassphrase)`] if `passphrase` is empty.
/// - [`SyncError::VaultError`] if reading secrets from the vault fails.
pub fn seal_artifact(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    passphrase: &str,
    envs: Option<&[&str]>,
) -> Result<SyncArtifact, SyncError> {
    // Validate passphrase early — before any vault I/O.
    if passphrase.trim().is_empty() {
        return Err(SyncError::Artifact(ArtifactError::WeakPassphrase));
    }

    // Determine which environment names to seal.
    let env_names: Vec<String> = match envs {
        Some(list) => list.iter().map(|s| s.to_lowercase()).collect(),
        None => vault
            .list_environments(project_id)
            .map_err(|e| SyncError::VaultError(e.to_string()))?
            .into_iter()
            .map(|e| e.name)
            .collect(),
    };

    let mut environments = BTreeMap::new();

    for env_name in &env_names {
        // Fetch all secrets for this environment (returns empty map if env doesn't exist yet).
        let secrets_map = crate::core::get_env_secrets(vault, master_key, project_id, env_name)
            .map_err(|e| SyncError::VaultError(e.to_string()))?;

        // get_env_secrets returns HashMap; ArtifactPayload requires BTreeMap for stable ordering.
        let secrets: BTreeMap<String, Zeroizing<String>> = secrets_map.into_iter().collect();
        let payload = ArtifactPayload { secrets };
        let envelope = seal_envelope(passphrase, &payload)?;
        environments.insert(env_name.clone(), envelope);
    }

    Ok(SyncArtifact {
        version: ARTIFACT_VERSION,
        environments,
    })
}

// ---------------------------------------------------------------------------
// T028 — unseal_artifact
// ---------------------------------------------------------------------------

/// Decrypts all accessible environments in `artifact` using `passphrase`.
///
/// Iterates every environment independently. Authentication failures (wrong
/// passphrase OR tampered ciphertext) add the environment to `skipped` —
/// the overall operation is never aborted. This is the Progressive Disclosure
/// contract: callers with a partial key import what they can access.
///
/// # Errors
/// - [`SyncError::Artifact(ArtifactError::WeakPassphrase)`] if `passphrase` is empty.
/// - [`SyncError::UnsupportedVersion`] if `artifact.version != ARTIFACT_VERSION`.
pub fn unseal_artifact(
    artifact: &SyncArtifact,
    passphrase: &str,
) -> Result<UnsealResult, SyncError> {
    if passphrase.trim().is_empty() {
        return Err(SyncError::Artifact(ArtifactError::WeakPassphrase));
    }
    if artifact.version != ARTIFACT_VERSION {
        return Err(SyncError::UnsupportedVersion(artifact.version));
    }

    let mut imported: BTreeMap<String, BTreeMap<String, Zeroizing<String>>> = BTreeMap::new();
    let mut skipped: Vec<String> = Vec::new();

    for (env_name, envelope) in &artifact.environments {
        match unseal_envelope(passphrase, env_name, envelope) {
            Ok(payload) => {
                imported.insert(env_name.clone(), payload.secrets);
            }
            Err(_) => {
                // Progressive Disclosure: ALL errors → graceful skip, never abort.
                skipped.push(env_name.clone());
            }
        }
    }

    Ok(UnsealResult { imported, skipped })
}

// ---------------------------------------------------------------------------
// T029 — write_artifact
// ---------------------------------------------------------------------------

/// Serializes `artifact` to pretty-printed JSON and writes it to `path`.
///
/// Keys are serialized in alphabetical order (guaranteed by [`BTreeMap`]).
/// Overwrites any existing file at `path`.
///
/// # Errors
/// - [`SyncError::Io`] on serialization or file write failure.
pub fn write_artifact(artifact: &SyncArtifact, path: &Path) -> Result<(), SyncError> {
    let json = serde_json::to_string_pretty(artifact).map_err(|e| SyncError::Io(e.to_string()))?;
    std::fs::write(path, json.as_bytes()).map_err(|e| SyncError::Io(e.to_string()))
}

// ---------------------------------------------------------------------------
// T030 — read_artifact
// ---------------------------------------------------------------------------

/// Reads and parses `envy.enc` from `path`.
///
/// Validates the top-level `version` field equals [`ARTIFACT_VERSION`].
///
/// # Errors
/// - [`SyncError::FileNotFound`] if `path` does not exist.
/// - [`SyncError::Io`] on read failure.
/// - [`SyncError::Artifact(ArtifactError::MalformedArtifact)`] if the JSON is invalid.
/// - [`SyncError::UnsupportedVersion`] if `version != ARTIFACT_VERSION`.
pub fn read_artifact(path: &Path) -> Result<SyncArtifact, SyncError> {
    if !path.exists() {
        return Err(SyncError::FileNotFound(path.display().to_string()));
    }
    let content = std::fs::read_to_string(path).map_err(|e| SyncError::Io(e.to_string()))?;
    let artifact: SyncArtifact = serde_json::from_str(&content)
        .map_err(|e| SyncError::Artifact(ArtifactError::MalformedArtifact(e.to_string())))?;
    if artifact.version != ARTIFACT_VERSION {
        return Err(SyncError::UnsupportedVersion(artifact.version));
    }
    Ok(artifact)
}

// ---------------------------------------------------------------------------
// Tests (T019–T023)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::artifact::{ArtifactPayload, EncryptedEnvelope, seal_envelope};
    use crate::db::{ProjectId, Vault};
    use std::collections::BTreeMap;
    use zeroize::Zeroizing;

    const TEST_KEY: [u8; 32] = [7u8; 32];

    /// Opens a temp vault, creates a project, and returns (vault, project_id).
    fn open_test_vault(tmp: &tempfile::TempDir) -> (Vault, ProjectId) {
        let path = tmp.path().join("vault.db");
        let vault = Vault::open(&path, &TEST_KEY).expect("vault must open");
        let pid = vault
            .create_project("test-project")
            .expect("project must be created");
        (vault, pid)
    }

    /// Builds an EncryptedEnvelope for `env_name` sealed with `passphrase`.
    fn make_envelope(passphrase: &str, key: &str, value: &str) -> EncryptedEnvelope {
        let mut secrets = BTreeMap::new();
        secrets.insert(key.to_string(), Zeroizing::new(value.to_string()));
        let payload = ArtifactPayload { secrets };
        seal_envelope(passphrase, &payload).expect("seal_envelope must succeed")
    }

    // T019 — seal_artifact_produces_valid_json_structure
    #[test]
    fn seal_artifact_produces_valid_json_structure() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        crate::core::set_secret(
            &vault,
            &TEST_KEY,
            &pid,
            "development",
            "STRIPE_KEY",
            "sk_test",
        )
        .expect("set_secret must succeed");

        let artifact = seal_artifact(&vault, &TEST_KEY, &pid, "team-pass", Some(&["development"]))
            .expect("seal_artifact must succeed");

        assert_eq!(artifact.version, ARTIFACT_VERSION);
        assert!(
            artifact.environments.contains_key("development"),
            "must contain development"
        );
        let env = &artifact.environments["development"];
        assert!(!env.ciphertext.is_empty(), "ciphertext must be non-empty");
        assert!(!env.nonce.is_empty(), "nonce must be non-empty");
        assert_eq!(env.kdf.algorithm, "argon2id");
    }

    // T020 — unseal_artifact_progressive_disclosure
    #[test]
    fn unseal_artifact_progressive_disclosure() {
        let dev_envelope = make_envelope("dev-pass", "API_KEY", "dev-value");
        let prod_envelope = make_envelope("prod-pass", "DB_URL", "prod-db");

        let mut environments = BTreeMap::new();
        environments.insert("development".to_string(), dev_envelope);
        environments.insert("production".to_string(), prod_envelope);

        let artifact = SyncArtifact {
            version: ARTIFACT_VERSION,
            environments,
        };

        let result = unseal_artifact(&artifact, "dev-pass").expect("unseal must succeed");

        assert!(
            result.imported.contains_key("development"),
            "development must be imported"
        );
        assert!(
            result.skipped.contains(&"production".to_string()),
            "production must be skipped"
        );
        assert_eq!(result.imported.len(), 1);
        assert_eq!(result.skipped.len(), 1);
    }

    // T021 — write_read_artifact_round_trip
    #[test]
    fn write_read_artifact_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = tmp.path().join("envy.enc");

        let dev_envelope = make_envelope("pass", "KEY", "val");
        let mut environments = BTreeMap::new();
        environments.insert("development".to_string(), dev_envelope);

        let artifact = SyncArtifact {
            version: ARTIFACT_VERSION,
            environments,
        };

        write_artifact(&artifact, &path).expect("write must succeed");
        let recovered = read_artifact(&path).expect("read must succeed");

        assert_eq!(recovered.version, ARTIFACT_VERSION);
        assert!(recovered.environments.contains_key("development"));
    }

    // T022 — read_artifact_malformed_json_returns_error
    #[test]
    fn read_artifact_malformed_json_returns_error() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = tmp.path().join("envy.enc");
        std::fs::write(&path, b"not json at all").expect("write must succeed");

        let result = read_artifact(&path);
        assert!(
            matches!(
                result,
                Err(SyncError::Artifact(ArtifactError::MalformedArtifact(_)))
            ),
            "malformed JSON must return MalformedArtifact, got: {:?}",
            result.err()
        );
    }

    // T023 — read_artifact_unknown_version_returns_error
    #[test]
    fn read_artifact_unknown_version_returns_error() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = tmp.path().join("envy.enc");
        std::fs::write(&path, b"{\"version\":999,\"environments\":{}}")
            .expect("write must succeed");

        let result = read_artifact(&path);
        assert!(
            matches!(result, Err(SyncError::UnsupportedVersion(999))),
            "unknown version must return UnsupportedVersion(999), got: {:?}",
            result.err()
        );
    }
}
