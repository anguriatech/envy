//! Secret CRUD operations and bulk environment decryption.
//!
//! This module is the orchestration hub of the Core layer. It:
//! - Validates all inputs before touching the DB or Crypto layers.
//! - Coordinates encrypt→store and fetch→decrypt sequences.
//! - Enforces the memory-safety contract: every returned plaintext lives inside
//!   a [`Zeroizing`] wrapper that zeroes its backing memory on drop.
//! - Auto-creates the target environment on write operations only.

use std::collections::HashMap;

use zeroize::Zeroizing;

use crate::db::{DbError, ProjectId, Vault};

use super::CoreError;

// ---------------------------------------------------------------------------
// T024 — DEFAULT_ENV constant + validate_key helper
// ---------------------------------------------------------------------------

/// The environment name used when no explicit environment is provided.
pub const DEFAULT_ENV: &str = "development";

/// Validates a secret key name before any DB or Crypto call.
///
/// A valid key is non-empty and contains no `=` character.
/// Returns [`CoreError::InvalidSecretKey`] on violation so the caller can
/// surface a precise error without leaking the key value into log contexts.
fn validate_key(key: &str) -> Result<(), CoreError> {
    if key.is_empty() || key.contains('=') {
        return Err(CoreError::InvalidSecretKey(key.to_owned()));
    }
    Ok(())
}

/// Normalises an environment name: defaults to [`DEFAULT_ENV`] when empty,
/// then lowercases (defense-in-depth against callers not normalising first).
fn normalize_env(env_name: &str) -> String {
    if env_name.is_empty() {
        DEFAULT_ENV.to_owned()
    } else {
        env_name.to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// T025 — resolve_env helper [US4]
// ---------------------------------------------------------------------------

/// Returns the [`EnvId`] for the given environment, creating it if absent.
///
/// This auto-create behaviour is intentionally limited to write operations
/// (`set_secret`). Read operations call `get_environment_by_name` directly
/// so that they return `DbError::NotFound` on a missing environment instead
/// of silently creating an empty one.
fn resolve_env(
    vault: &Vault,
    project_id: &ProjectId,
    env_name: &str,
) -> Result<crate::db::EnvId, CoreError> {
    let name = normalize_env(env_name);
    match vault.get_environment_by_name(project_id, &name) {
        Ok(env) => Ok(env.id),
        Err(DbError::NotFound) => Ok(vault.create_environment(project_id, &name)?),
        Err(e) => Err(CoreError::Db(e)),
    }
}

// ---------------------------------------------------------------------------
// T026 — set_secret [US2]
// ---------------------------------------------------------------------------

/// Encrypts `plaintext` and stores it in the vault under `(project_id, env_name, key)`.
///
/// Upsert semantics: a second call with the same key replaces the existing
/// ciphertext. The environment is auto-created if it does not yet exist.
/// Passing an empty `env_name` uses [`DEFAULT_ENV`] ("development").
///
/// # Errors
/// - [`CoreError::InvalidSecretKey`] if `key` is empty or contains `=`.
/// - [`CoreError::Crypto`] on encryption failure (structurally impossible
///   with a valid 32-byte key).
/// - [`CoreError::Db`] on any database write failure.
pub fn set_secret(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
    key: &str,
    plaintext: &str,
) -> Result<(), CoreError> {
    validate_key(key)?;
    let env_id = resolve_env(vault, project_id, env_name)?;
    let encrypted = crate::crypto::encrypt(master_key, plaintext.as_bytes())?;
    vault.upsert_secret(&env_id, key, &encrypted.ciphertext, &encrypted.nonce)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T027 — get_secret [US2]
// ---------------------------------------------------------------------------

/// Fetches and decrypts the secret for `(project_id, env_name, key)`.
///
/// Returns the plaintext wrapped in [`Zeroizing`] — the backing memory is
/// zeroed when the caller drops the value. Read operations do NOT auto-create
/// the environment; they return `Db(NotFound)` if it is missing.
///
/// # Errors
/// - [`CoreError::InvalidSecretKey`] if `key` is empty or contains `=`.
/// - [`CoreError::Db`] if the environment, key, or project does not exist.
/// - [`CoreError::Crypto`] if the GCM authentication tag does not verify.
pub fn get_secret(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
    key: &str,
) -> Result<Zeroizing<String>, CoreError> {
    validate_key(key)?;
    let name = normalize_env(env_name);
    let env = vault.get_environment_by_name(project_id, &name)?;
    let record = vault.get_secret(&env.id, key)?;
    let plaintext_bytes =
        crate::crypto::decrypt(master_key, &record.value_encrypted, &record.value_nonce)?;
    // `to_vec()` creates a brief intermediate copy; the original Zeroizing<Vec<u8>>
    // zeroes its bytes on drop (end of this statement). The copy lives only until
    // String::from_utf8 consumes it and becomes the Zeroizing<String> return value.
    let string = String::from_utf8(plaintext_bytes.to_vec())
        .map_err(|_| CoreError::Crypto(crate::crypto::CryptoError::DecryptionFailed))?;
    Ok(Zeroizing::new(string))
}

// ---------------------------------------------------------------------------
// T028 — list_secret_keys [US2]
// ---------------------------------------------------------------------------

/// Returns the key names for all secrets in the environment, ordered alphabetically.
///
/// Does NOT decrypt values — safe to call without the master key. Returns an
/// empty `Vec` if the environment exists but has no secrets.
///
/// # Errors
/// - [`CoreError::Db`] if the environment or project does not exist.
pub fn list_secret_keys(
    vault: &Vault,
    project_id: &ProjectId,
    env_name: &str,
) -> Result<Vec<String>, CoreError> {
    let name = normalize_env(env_name);
    let env = vault.get_environment_by_name(project_id, &name)?;
    // `list_secrets` returns rows ORDER BY key ASC, so no sort needed here.
    Ok(vault
        .list_secrets(&env.id)?
        .into_iter()
        .map(|r| r.key)
        .collect())
}

// ---------------------------------------------------------------------------
// list_secrets_with_values [008-output-formats]
// ---------------------------------------------------------------------------

/// Decrypts ALL secrets for the given environment and returns them as ordered
/// `(key, plaintext_value)` pairs.
///
/// Pairs are ordered alphabetically by key (consistent with [`list_secret_keys`]).
/// If the environment has no secrets, returns an empty `Vec`.
/// Read operations do NOT auto-create the environment.
///
/// # Errors
/// - [`CoreError::Db`] if the environment or project does not exist.
/// - [`CoreError::Crypto`] if any ciphertext fails GCM authentication.
pub fn list_secrets_with_values(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
) -> Result<Vec<(String, String)>, CoreError> {
    let name = normalize_env(env_name);
    let env = vault.get_environment_by_name(project_id, &name)?;
    // list_secrets returns records ORDER BY key ASC — no additional sort needed.
    let records = vault.list_secrets(&env.id)?;
    let mut pairs = Vec::with_capacity(records.len());
    for record in records {
        let plaintext_bytes =
            crate::crypto::decrypt(master_key, &record.value_encrypted, &record.value_nonce)?;
        let string = String::from_utf8(plaintext_bytes.to_vec())
            .map_err(|_| CoreError::Crypto(crate::crypto::CryptoError::DecryptionFailed))?;
        pairs.push((record.key, string));
    }
    Ok(pairs)
}

// ---------------------------------------------------------------------------
// T029 — delete_secret [US2]
// ---------------------------------------------------------------------------

/// Permanently deletes the secret for `(project_id, env_name, key)`.
///
/// Returns `Db(NotFound)` if the key does not exist; no mutation occurs.
///
/// # Errors
/// - [`CoreError::InvalidSecretKey`] if `key` is empty or contains `=`.
/// - [`CoreError::Db`] if the environment, key, or project does not exist.
pub fn delete_secret(
    vault: &Vault,
    project_id: &ProjectId,
    env_name: &str,
    key: &str,
) -> Result<(), CoreError> {
    validate_key(key)?;
    let name = normalize_env(env_name);
    let env = vault.get_environment_by_name(project_id, &name)?;
    vault.delete_secret(&env.id, key)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T030 — get_env_secrets [US3]
// ---------------------------------------------------------------------------

/// Decrypts ALL secrets for the given environment.
///
/// Returns a `HashMap<String, Zeroizing<String>>` ready for child-process
/// environment injection. If **any** single decryption fails, the entire
/// operation fails — no partial map is ever returned.
///
/// An empty environment returns `Ok(HashMap::new())`, not an error. Read
/// operations do NOT auto-create the environment.
///
/// # Errors
/// - [`CoreError::Db`] if the environment or project does not exist.
/// - [`CoreError::Crypto`] if any ciphertext fails GCM authentication.
pub fn get_env_secrets(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
) -> Result<HashMap<String, Zeroizing<String>>, CoreError> {
    let name = normalize_env(env_name);
    // A missing environment means no secrets have been set yet — return empty map
    // rather than propagating NotFound. This makes `envy run` work on a fresh project.
    let env = match vault.get_environment_by_name(project_id, &name) {
        Ok(env) => env,
        Err(DbError::NotFound) => return Ok(HashMap::new()),
        Err(e) => return Err(CoreError::Db(e)),
    };
    let records = vault.list_secrets(&env.id)?;
    let mut map = HashMap::new();
    for record in records {
        // `?` here makes the whole loop short-circuit on any decrypt failure,
        // guaranteeing atomicity: no partial map is ever returned to the caller.
        let plaintext_bytes =
            crate::crypto::decrypt(master_key, &record.value_encrypted, &record.value_nonce)?;
        let string = String::from_utf8(plaintext_bytes.to_vec())
            .map_err(|_| CoreError::Crypto(crate::crypto::CryptoError::DecryptionFailed))?;
        map.insert(record.key, Zeroizing::new(string));
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// T013–T023 — Tests (written first to define the contract; compile-failed before T024–T030)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CoreError;
    use crate::db::{ProjectId, Vault};

    /// Opens a temp-file vault with a 32-byte all-zero key and creates one project.
    fn open_test_vault(tmp: &tempfile::TempDir) -> (Vault, ProjectId) {
        let path = tmp.path().join("test.vault");
        let vault = Vault::open(&path, &[0u8; 32]).expect("open vault");
        let pid = vault
            .create_project("test-project")
            .expect("create project");
        (vault, pid)
    }

    // T013
    #[test]
    fn set_and_get_secret_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        set_secret(&vault, &key, &pid, "test", "API_KEY", "secret123").expect("set must succeed");

        // Raw DB bytes must not contain the plaintext.
        let env = vault
            .get_environment_by_name(&pid, "test")
            .expect("env must exist");
        let record = vault
            .get_secret(&env.id, "API_KEY")
            .expect("record must exist");
        assert!(
            !record.value_encrypted.windows(9).any(|w| w == b"secret123"),
            "plaintext must not appear in ciphertext"
        );

        // Decryption round-trip must yield the original value.
        let plaintext =
            get_secret(&vault, &key, &pid, "test", "API_KEY").expect("get must succeed");
        assert_eq!(plaintext.as_str(), "secret123");
    }

    // T014
    #[test]
    fn set_secret_upsert() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        set_secret(&vault, &key, &pid, "test", "API_KEY", "v1").expect("first set");
        set_secret(&vault, &key, &pid, "test", "API_KEY", "v2").expect("second set");

        let plaintext =
            get_secret(&vault, &key, &pid, "test", "API_KEY").expect("get must succeed");
        assert_eq!(
            plaintext.as_str(),
            "v2",
            "last write must win (upsert semantics)"
        );
    }

    // T015
    #[test]
    fn get_secret_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        // Create the env so the error is specifically about the missing key.
        vault.create_environment(&pid, "test").expect("create env");

        let result = get_secret(&vault, &key, &pid, "test", "NONEXISTENT");
        assert!(
            matches!(result, Err(CoreError::Db(_))),
            "missing key must return Db error, got: {:?}",
            result
        );
    }

    // T016
    #[test]
    fn list_secret_keys_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        set_secret(&vault, &key, &pid, "test", "ZEBRA", "z").expect("set ZEBRA");
        set_secret(&vault, &key, &pid, "test", "ALPHA", "a").expect("set ALPHA");
        set_secret(&vault, &key, &pid, "test", "MANGO", "m").expect("set MANGO");

        let keys = list_secret_keys(&vault, &pid, "test").expect("list must succeed");
        assert_eq!(
            keys,
            vec!["ALPHA", "MANGO", "ZEBRA"],
            "keys must be alphabetical"
        );
        assert_eq!(keys.len(), 3);
    }

    // T017
    #[test]
    fn delete_secret_removes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        set_secret(&vault, &key, &pid, "test", "TOKEN", "abc").expect("set");
        delete_secret(&vault, &pid, "test", "TOKEN").expect("delete must succeed");

        let result = get_secret(&vault, &key, &pid, "test", "TOKEN");
        assert!(
            matches!(result, Err(CoreError::Db(_))),
            "key must be absent after delete, got: {:?}",
            result
        );
    }

    // T018
    #[test]
    fn delete_secret_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);

        vault.create_environment(&pid, "test").expect("create env");

        let result = delete_secret(&vault, &pid, "test", "NONEXISTENT");
        assert!(
            matches!(result, Err(CoreError::Db(_))),
            "deleting a non-existent key must return Db error, got: {:?}",
            result
        );
    }

    // T019
    #[test]
    fn get_env_secrets_all_decrypted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        set_secret(&vault, &key, &pid, "test", "A", "1").expect("set A");
        set_secret(&vault, &key, &pid, "test", "B", "2").expect("set B");
        set_secret(&vault, &key, &pid, "test", "C", "3").expect("set C");

        let secrets = get_env_secrets(&vault, &key, &pid, "test").expect("must succeed");
        assert_eq!(secrets.len(), 3, "map must have exactly 3 entries");
        assert_eq!(secrets.get("A").map(|v| v.as_str()), Some("1"));
        assert_eq!(secrets.get("B").map(|v| v.as_str()), Some("2"));
        assert_eq!(secrets.get("C").map(|v| v.as_str()), Some("3"));
    }

    // Bug regression: get_env_secrets on a fresh project (environment never created)
    // must return Ok(empty map), not Err — `envy run` crashed on first use.
    #[test]
    fn get_env_secrets_missing_env_returns_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);

        // Fresh project — no environments created, no secrets set whatsoever.
        let result = get_env_secrets(&vault, &[0u8; 32], &pid, "development");
        assert!(
            result.is_ok(),
            "missing environment must return Ok, not Err; got: {:?}",
            result
        );
        assert!(
            result.unwrap().is_empty(),
            "result must be an empty map for a fresh project"
        );
    }

    // T020
    #[test]
    fn get_env_secrets_empty_env() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);

        // Create the environment but add no secrets.
        vault.create_environment(&pid, "empty").expect("create env");

        let secrets = get_env_secrets(&vault, &[0u8; 32], &pid, "empty")
            .expect("must succeed even with no secrets");
        assert!(
            secrets.is_empty(),
            "expected empty map, got {} entries",
            secrets.len()
        );
    }

    // T021
    #[test]
    fn get_env_secrets_partial_fail() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        // One valid secret via the public API.
        set_secret(&vault, &key, &pid, "test", "VALID", "hello").expect("set VALID");

        // A second secret inserted directly via the DB layer with garbage ciphertext.
        // Nonce is structurally valid (12 bytes) but the ciphertext body will fail
        // GCM authentication tag verification during decryption.
        let env = vault
            .get_environment_by_name(&pid, "test")
            .expect("env must exist after set_secret");
        vault
            .upsert_secret(
                &env.id,
                "CORRUPTED",
                b"not-valid-gcm-ciphertext-xxxxx",
                &[0u8; 12],
            )
            .expect("upsert raw garbage ciphertext");

        let result = get_env_secrets(&vault, &key, &pid, "test");
        assert!(
            matches!(result, Err(CoreError::Crypto(_))),
            "corrupted ciphertext must return Crypto error; got: {:?}",
            result
        );
    }

    // T022
    #[test]
    fn default_env_auto_created() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        // Empty env_name must silently default to "development".
        set_secret(&vault, &key, &pid, "", "API_KEY", "abc")
            .expect("set with empty env_name must succeed");

        // Verify the "development" environment was auto-created.
        let env = vault
            .get_environment_by_name(&pid, "development")
            .expect("\"development\" environment must have been auto-created");
        vault
            .get_secret(&env.id, "API_KEY")
            .expect("secret must exist in development env");
    }

    // T023
    #[test]
    fn invalid_key_rejected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (vault, pid) = open_test_vault(&tmp);
        let key = [0u8; 32];

        let result = set_secret(&vault, &key, &pid, "test", "", "value");
        assert!(
            matches!(result, Err(CoreError::InvalidSecretKey(_))),
            "empty key must return InvalidSecretKey, got: {:?}",
            result
        );

        let result = set_secret(&vault, &key, &pid, "test", "FOO=BAR", "value");
        assert!(
            matches!(result, Err(CoreError::InvalidSecretKey(_))),
            "key with '=' must return InvalidSecretKey, got: {:?}",
            result
        );

        // validate_key fires before any DB access — no environments must be created.
        let envs = vault.list_environments(&pid).expect("list environments");
        assert!(
            envs.is_empty(),
            "no environments must be created for invalid keys; got {} envs",
            envs.len()
        );
    }
}
