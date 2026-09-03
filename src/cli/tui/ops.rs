use super::app::SecretEntry;
use crate::{
    cli::CliError,
    core, crypto,
    db::{ProjectId, Vault},
};
use std::collections::BTreeMap;
use std::path::Path;
use zeroize::Zeroizing;

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
) -> Result<(), CliError> {
    let mut artifact = match core::read_artifact(artifact_path) {
        Ok(artifact) => artifact,
        Err(core::SyncError::FileNotFound(_)) => core::new_empty_artifact(),
        Err(error) => return Err(CliError::ArtifactUnreadable(error.to_string())),
    };
    if let Some(existing) = artifact.environments.get(environment)
        && !core::check_envelope_passphrase(passphrase, environment, existing)
    {
        return Err(CliError::Output(format!(
            "incorrect passphrase for environment '{environment}'; run `envy rotate`"
        )));
    }
    let envelope = core::seal_env_unmarked(vault, key, project, environment, passphrase)
        .map_err(|error| CliError::Output(error.to_string()))?;
    artifact
        .environments
        .insert(environment.to_ascii_lowercase(), envelope);
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
        assert!(mismatch.to_string().contains("envy rotate"));

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
        assert!(error.to_string().contains("failed to read/write"));
        let status = vault.environment_status(&project).expect("status");
        assert!(status[0].sealed_at.is_none());
    }

    #[test]
    fn specific_passphrase_takes_precedence_over_global() {
        let resolved = resolve_passphrase_values(Some("specific".into()), Some("global".into()))
            .expect("passphrase");
        assert_eq!(resolved.as_str(), "specific");
    }
}
