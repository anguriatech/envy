use super::app::SecretEntry;
use crate::{
    cli::CliError,
    core, crypto,
    db::{ProjectId, Vault},
};
use std::collections::BTreeMap;
use std::path::Path;
use zeroize::Zeroizing;

/// Distinguishes a passphrase mismatch (recoverable in-TUI through rotate or
/// import) from every other seal failure, which stay opaque `CliError`s.
#[derive(Debug, thiserror::Error)]
pub enum SealError {
    #[error(
        "passphrase does not match the existing envelope for '{0}' — press R to rotate or Y to import"
    )]
    Mismatch(String),

    #[error("{0}")]
    Other(#[from] CliError),
}

pub fn load_projects(vault: &Vault) -> Result<Vec<core::ProjectSummary>, CliError> {
    core::list_projects(vault).map_err(CliError::Core)
}
pub fn load_environments(
    vault: &Vault,
    project: &ProjectId,
) -> Result<Vec<core::EnvironmentSummary>, CliError> {
    core::list_environments(vault, project).map_err(CliError::Core)
}
pub fn load_secrets(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    env: &str,
) -> Result<Vec<SecretEntry>, CliError> {
    core::list_secrets_with_metadata(vault, key, project, env)
        .map(|values| {
            values
                .into_iter()
                .map(|secret| SecretEntry {
                    key: secret.key,
                    value: secret.value,
                    updated_at: secret.updated_at,
                    revealed: false,
                })
                .collect()
        })
        .map_err(CliError::Core)
}

pub fn set_secret(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    env: &str,
    name: &str,
    value: &str,
) -> Result<(), CliError> {
    core::set_secret(vault, key, project, env, name, value).map_err(CliError::Core)
}
pub fn delete_secret(
    vault: &Vault,
    project: &ProjectId,
    env: &str,
    name: &str,
) -> Result<(), CliError> {
    core::delete_secret(vault, project, env, name).map_err(CliError::Core)
}

pub fn delete_project(vault: &Vault, project: &ProjectId) -> Result<(), CliError> {
    core::delete_project(vault, project).map_err(CliError::Core)
}

pub fn project_deletion_counts(
    vault: &Vault,
    project: &ProjectId,
) -> Result<(usize, usize), CliError> {
    core::project_deletion_counts(vault, project).map_err(CliError::Core)
}

pub fn environment_has_secrets(
    vault: &Vault,
    project: &ProjectId,
    environment: &str,
) -> Result<bool, CliError> {
    core::list_secret_keys(vault, project, environment)
        .map(|keys| !keys.is_empty())
        .map_err(CliError::Core)
}

/// Number of secrets in an environment (0 when the environment is missing).
pub fn count_secrets(
    vault: &Vault,
    project: &ProjectId,
    environment: &str,
) -> Result<usize, CliError> {
    match core::list_secret_keys(vault, project, environment) {
        Ok(keys) => Ok(keys.len()),
        Err(crate::core::CoreError::Db(crate::db::DbError::NotFound)) => Ok(0),
        Err(error) => Err(CliError::Core(error)),
    }
}

pub fn status_report(
    vault: &Vault,
    project: &ProjectId,
    rotation_reminder_days: u32,
) -> Result<Vec<core::StatusRow>, CliError> {
    core::get_status_report(vault, project, rotation_reminder_days).map_err(CliError::Core)
}

pub fn env_diff(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    environment: &str,
    artifact_path: &Path,
    passphrase: &str,
) -> Result<String, CliError> {
    let vault_map: BTreeMap<String, Zeroizing<String>> =
        core::get_env_secrets(vault, key, project, environment)
            .map_err(CliError::Core)?
            .into_iter()
            .collect();

    let mut artifact_map: BTreeMap<String, Zeroizing<String>> = BTreeMap::new();
    match core::read_artifact(artifact_path) {
        Err(core::SyncError::FileNotFound(_)) => {}
        Err(error) => return Err(CliError::ArtifactUnreadable(error.to_string())),
        Ok(artifact) => {
            if artifact.environments.contains_key(environment) {
                match core::unseal_env(&artifact, environment, passphrase)
                    .map_err(|error| CliError::Output(error.to_string()))?
                {
                    Some(secrets) => artifact_map = secrets,
                    None => {
                        return Err(CliError::Output(format!(
                            "incorrect passphrase for environment '{environment}'"
                        )));
                    }
                }
            }
        }
    }

    let report = core::compute_diff(environment, vault_map, artifact_map);
    if !report.has_differences() {
        return Ok(format!("envy diff: {environment} — no differences"));
    }
    let mut text = format!(
        "envy diff: {environment} (vault ↔ envy.enc)\n\n\
         {} added, {} removed, {} modified\n",
        report.added, report.removed, report.modified
    );
    for entry in &report.entries {
        let symbol = match entry.change {
            core::ChangeType::Added => '+',
            core::ChangeType::Removed => '-',
            core::ChangeType::Modified => '~',
        };
        text.push_str(&format!("  {symbol} {}\n", entry.key));
    }
    Ok(text)
}

pub fn decrypt_env(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    environment: &str,
    artifact_path: &Path,
    passphrase: &str,
) -> Result<usize, CliError> {
    let artifact = match core::read_artifact(artifact_path) {
        Ok(a) => a,
        Err(core::SyncError::FileNotFound(_)) => {
            return Err(CliError::FileNotFound(
                artifact_path.display().to_string(),
                "envy.enc not found".into(),
            ));
        }
        Err(error) => return Err(CliError::ArtifactUnreadable(error.to_string())),
    };
    match core::unseal_env(&artifact, environment, passphrase) {
        Ok(Some(secrets)) => {
            let mut count = 0;
            for (k, v) in &secrets {
                core::set_secret(vault, key, project, environment, k, v).map_err(CliError::Core)?;
                count += 1;
            }
            Ok(count)
        }
        Ok(None) => Err(CliError::Output(format!(
            "environment '{environment}' not found in artifact or wrong passphrase"
        ))),
        Err(error) => Err(CliError::Output(error.to_string())),
    }
}

pub fn resolve_passphrase(environment: &str) -> Option<Zeroizing<String>> {
    let variable = format!("ENVY_PASSPHRASE_{}", environment.to_ascii_uppercase());
    resolve_passphrase_values(
        std::env::var(&variable).ok(),
        std::env::var("ENVY_PASSPHRASE").ok(),
    )
}

fn resolve_passphrase_values(
    specific: Option<String>,
    global: Option<String>,
) -> Option<Zeroizing<String>> {
    specific.or(global).map(Zeroizing::new)
}

pub fn open_vault() -> Result<(Vault, Zeroizing<[u8; 32]>), CliError> {
    let key = crypto::get_or_create_master_key()
        .map_err(|error| CliError::VaultOpen(error.to_string()))?;
    let vault = Vault::open(&super::super::vault_path(), key.as_ref())
        .map_err(|error| CliError::VaultOpen(error.to_string()))?;
    Ok((vault, key))
}

pub fn sync_environment(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    environment: &str,
    passphrase: &str,
    artifact_path: &Path,
) -> Result<(), SealError> {
    let mut artifact = match core::read_artifact(artifact_path) {
        Ok(artifact) => artifact,
        Err(core::SyncError::FileNotFound(_)) => core::new_empty_artifact(),
        Err(error) => {
            return Err(SealError::Other(CliError::ArtifactUnreadable(
                error.to_string(),
            )));
        }
    };
    // Vault environment names are lowercase (core::normalize_env); the artifact
    // keys them lowercased too, so the lookup normalizes for defense in depth.
    let envelope_key = environment.to_ascii_lowercase();
    if let Some(existing) = artifact.environments.get(&envelope_key)
        && !core::check_envelope_passphrase(passphrase, environment, existing)
    {
        return Err(SealError::Mismatch(envelope_key));
    }
    let envelope = core::seal_env_unmarked(vault, key, project, environment, passphrase)
        .map_err(|error| SealError::Other(CliError::Output(error.to_string())))?;
    artifact.environments.insert(envelope_key, envelope);
    core::write_artifact(&artifact, artifact_path)
        .map_err(|error| SealError::Other(CliError::Output(error.to_string())))?;
    core::mark_env_sealed(vault, project, environment)
        .map_err(|error| SealError::Other(CliError::Output(error.to_string())))
}

/// Re-seals `environment` in `envy.enc` with a new passphrase after verifying
/// `current`. The safe in-TUI path for the rotation dead-end: a wrong current
/// passphrase fails before the artifact is touched (core::rotate_env contract).
pub fn rotate_environment(
    vault: &Vault,
    key: &[u8; 32],
    project: &ProjectId,
    environment: &str,
    artifact_path: &Path,
    current: &str,
    new_pass: &str,
) -> Result<(), CliError> {
    let mut artifact = match core::read_artifact(artifact_path) {
        Ok(artifact) => artifact,
        Err(core::SyncError::FileNotFound(path)) => {
            return Err(CliError::FileNotFound(
                path,
                "envy.enc not found; seal the environment first".into(),
            ));
        }
        Err(error) => return Err(CliError::ArtifactUnreadable(error.to_string())),
    };
    core::rotate_env(
        vault,
        key,
        project,
        &mut artifact,
        environment,
        current,
        new_pass,
    )
    .map_err(|error| match error {
        core::SyncError::Artifact(crypto::artifact::ArtifactError::MalformedArtifact(_)) => {
            CliError::EnvNotFound(environment.to_owned())
        }
        core::SyncError::Artifact(crypto::artifact::ArtifactError::MalformedEnvelope(_, _)) => {
            CliError::PassphraseInput(
                "current passphrase does not match the existing envelope".into(),
            )
        }
        core::SyncError::Artifact(crypto::artifact::ArtifactError::WeakPassphrase) => {
            CliError::PassphraseInput("new passphrase must not be empty or whitespace".into())
        }
        other => CliError::Output(other.to_string()),
    })?;
    core::write_artifact(&artifact, artifact_path)
        .map_err(|error| CliError::Output(error.to_string()))?;
    core::mark_env_sealed(vault, project, environment)
        .map_err(|error| CliError::Output(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0xAB; 32];

    #[test]
    fn crud_round_trip_uses_core_operations() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("tui-test").expect("project");

        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "one").expect("create");
        let secrets = load_secrets(&vault, &TEST_KEY, &project, "development").expect("load");
        assert_eq!(secrets[0].value.as_str(), "one");
        assert!(secrets[0].updated_at > 0);
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "two").expect("edit");
        let secrets = load_secrets(&vault, &TEST_KEY, &project, "development").expect("load");
        assert_eq!(secrets[0].value.as_str(), "two");
        delete_secret(&vault, &project, "development", "TOKEN").expect("delete");
        assert!(
            load_secrets(&vault, &TEST_KEY, &project, "development")
                .expect("reload")
                .is_empty()
        );
    }

    #[test]
    fn invalid_secret_key_surfaces_core_error() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("tui-test").expect("project");
        let error = set_secret(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "BAD=KEY",
            "value",
        )
        .expect_err("invalid key must fail");
        assert!(error.to_string().contains("invalid secret key"));
    }

    #[test]
    fn deleting_project_cascades_its_environment_and_secrets() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("remove-me").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "value").expect("secret");
        assert_eq!(
            project_deletion_counts(&vault, &project).expect("counts"),
            (1, 1)
        );

        delete_project(&vault, &project).expect("delete project");
        assert!(load_projects(&vault).expect("projects").is_empty());
    }

    #[test]
    fn sync_environment_merges_multiple_environments() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("tui-test").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "DEV", "one").expect("dev");
        set_secret(&vault, &TEST_KEY, &project, "production", "PROD", "two").expect("prod");
        let artifact_path = temp.path().join("envy.enc");

        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "dev-pass",
            &artifact_path,
        )
        .expect("development sync");
        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "production",
            "prod-pass",
            &artifact_path,
        )
        .expect("production sync");
        let mismatch = sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "wrong-pass",
            &artifact_path,
        )
        .expect_err("wrong existing passphrase must fail");
        assert!(
            matches!(mismatch, SealError::Mismatch(ref env) if env == "development"),
            "mismatch must be distinguishable from other failures, got: {mismatch:?}"
        );

        let artifact = core::read_artifact(&artifact_path).expect("artifact");
        assert_eq!(artifact.environments.len(), 2);
        assert!(core::check_envelope_passphrase(
            "dev-pass",
            "development",
            &artifact.environments["development"]
        ));
        assert!(core::check_envelope_passphrase(
            "prod-pass",
            "production",
            &artifact.environments["production"]
        ));
    }

    #[test]
    fn first_seal_of_never_sealed_environment_succeeds() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("fresh").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "v").expect("secret");
        let artifact_path = temp.path().join("envy.enc");

        // Regression: sealing an environment that has never been encrypted must
        // not trip the existing-envelope passphrase check.
        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "first-pass",
            &artifact_path,
        )
        .expect("first seal must succeed");

        let artifact = core::read_artifact(&artifact_path).expect("artifact");
        assert!(artifact.environments.contains_key("development"));
    }

    #[test]
    fn sync_lookup_is_case_insensitive_against_artifact_keys() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("casing").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "v").expect("secret");
        let artifact_path = temp.path().join("envy.enc");

        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "same-pass",
            &artifact_path,
        )
        .expect("initial seal");
        // Mixed-case re-seal with the CORRECT passphrase must not be reported
        // as a mismatch (artifact keys are lowercased).
        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "Development",
            "same-pass",
            &artifact_path,
        )
        .expect("case-insensitive re-seal");
    }

    #[test]
    fn failed_artifact_write_does_not_commit_sync_marker() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("tui-test").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "value").expect("secret");
        let bad_path = temp.path().join("missing").join("envy.enc");

        let error = sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "passphrase",
            &bad_path,
        )
        .expect_err("write must fail");
        assert!(
            error.to_string().contains("failed to read/write"),
            "unexpected error: {error}"
        );
        let status = vault.environment_status(&project).expect("status");
        assert!(status[0].sealed_at.is_none());
    }

    #[test]
    fn count_secrets_reports_zero_for_missing_environment() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("counting").expect("project");

        assert_eq!(
            count_secrets(&vault, &project, "nowhere").expect("missing env counts as 0"),
            0
        );
        set_secret(&vault, &TEST_KEY, &project, "development", "A", "1").expect("A");
        set_secret(&vault, &TEST_KEY, &project, "development", "B", "2").expect("B");
        assert_eq!(
            count_secrets(&vault, &project, "development").expect("count"),
            2
        );
    }

    #[test]
    fn rotate_environment_error_mappings() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("rotate-errors").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "v").expect("secret");
        let artifact_path = temp.path().join("envy.enc");

        // Missing artifact → FileNotFound (exit-code 1 family).
        let missing = rotate_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            &artifact_path,
            "current",
            "new",
        )
        .expect_err("missing artifact must fail");
        assert!(
            missing.to_string().contains("envy.enc not found"),
            "unexpected error: {missing}"
        );

        // Artifact present but env not sealed in it → EnvNotFound.
        let empty_artifact = core::new_empty_artifact();
        core::write_artifact(&empty_artifact, &artifact_path).expect("write empty artifact");
        let absent = rotate_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            &artifact_path,
            "current",
            "new",
        )
        .expect_err("absent env must fail");
        assert!(
            matches!(absent, CliError::EnvNotFound(ref env) if env == "development"),
            "unexpected error: {absent}"
        );
    }

    #[test]
    fn rotate_environment_re_seals_with_new_passphrase() {
        let temp = tempfile::tempdir().expect("temp directory");
        let vault_path = temp.path().join("vault.db");
        let vault = Vault::open(&vault_path, &TEST_KEY).expect("vault");
        let project = vault.create_project("rotate-me").expect("project");
        set_secret(&vault, &TEST_KEY, &project, "development", "TOKEN", "v").expect("secret");
        let artifact_path = temp.path().join("envy.enc");

        sync_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            "old-pass",
            &artifact_path,
        )
        .expect("initial seal");

        rotate_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            &artifact_path,
            "old-pass",
            "new-pass",
        )
        .expect("rotate must succeed");

        let artifact = core::read_artifact(&artifact_path).expect("artifact");
        let envelope = &artifact.environments["development"];
        assert!(core::check_envelope_passphrase(
            "new-pass",
            "development",
            envelope
        ));
        assert!(!core::check_envelope_passphrase(
            "old-pass",
            "development",
            envelope
        ));

        let wrong_current = rotate_environment(
            &vault,
            &TEST_KEY,
            &project,
            "development",
            &artifact_path,
            "old-pass",
            "another",
        )
        .expect_err("wrong current passphrase must fail");
        assert!(
            wrong_current
                .to_string()
                .contains("current passphrase does not match"),
            "unexpected error: {wrong_current}"
        );
    }

    #[test]
    fn specific_passphrase_takes_precedence_over_global() {
        let resolved = resolve_passphrase_values(Some("specific".into()), Some("global".into()))
            .expect("passphrase");
        assert_eq!(resolved.as_str(), "specific");
    }
}
