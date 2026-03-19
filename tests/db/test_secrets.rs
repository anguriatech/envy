// Integration tests for the db layer — secret upsert, overwrite, cascade deletes.

use envy::db::{DbError, EnvId, Vault};

const DUMMY_KEY: [u8; 32] = [0u8; 32];
const NONCE_12: [u8; 12] = [1u8; 12];

fn open_temp_vault() -> (Vault, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault = Vault::open(tmp.path(), &DUMMY_KEY)
        .expect("Vault::open on a fresh temp file always succeeds");
    (vault, tmp)
}

/// Creates a project + environment and returns the EnvId.
fn make_env(vault: &Vault) -> EnvId {
    let project_id = vault
        .create_project("test-project")
        .expect("create_project must succeed in setup");
    vault
        .create_environment(&project_id, "development")
        .expect("create_environment must succeed in setup")
}

// ---------------------------------------------------------------------------
// (a) upsert_secret succeeds with valid 12-byte nonce
// ---------------------------------------------------------------------------

#[test]
fn test_upsert_secret_succeeds() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    let ciphertext = b"fake-ciphertext-bytes";
    let secret_id = vault
        .upsert_secret(&env_id, "DATABASE_URL", ciphertext, &NONCE_12)
        .expect("upsert_secret must succeed with a 12-byte nonce");

    assert_eq!(
        secret_id.as_str().len(),
        36,
        "SecretId must be a 36-character UUID"
    );
}

// ---------------------------------------------------------------------------
// (b) get_secret returns byte-for-byte identical ciphertext and nonce
// ---------------------------------------------------------------------------

#[test]
fn test_get_secret_returns_exact_bytes() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    let ciphertext = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE];
    let nonce = [0xAB; 12];

    vault
        .upsert_secret(&env_id, "MY_KEY", &ciphertext, &nonce)
        .expect("upsert_secret must succeed");

    let record = vault
        .get_secret(&env_id, "MY_KEY")
        .expect("get_secret must find the just-upserted secret");

    assert_eq!(record.key, "MY_KEY");
    assert_eq!(
        record.value_encrypted, ciphertext,
        "value_encrypted must match byte-for-byte"
    );
    assert_eq!(
        record.value_nonce,
        nonce.to_vec(),
        "value_nonce must match byte-for-byte"
    );
    assert_eq!(record.environment_id, env_id);
    assert!(record.created_at > 0, "created_at must be a non-zero epoch");
    assert!(record.updated_at > 0, "updated_at must be a non-zero epoch");
}

// ---------------------------------------------------------------------------
// (c) second upsert for same key replaces the record
// ---------------------------------------------------------------------------

#[test]
fn test_upsert_secret_replaces_existing() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    vault
        .upsert_secret(&env_id, "API_KEY", b"old-ciphertext", &NONCE_12)
        .expect("first upsert must succeed");

    let new_ciphertext = b"new-ciphertext-v2";
    vault
        .upsert_secret(&env_id, "API_KEY", new_ciphertext, &NONCE_12)
        .expect("second upsert must succeed");

    let record = vault
        .get_secret(&env_id, "API_KEY")
        .expect("get_secret must return the updated record");

    assert_eq!(
        record.value_encrypted, new_ciphertext,
        "get_secret after second upsert must return the new ciphertext"
    );
}

// ---------------------------------------------------------------------------
// (d) upsert with nonce != 12 bytes returns DbError::ConstraintViolation
// ---------------------------------------------------------------------------

#[test]
fn test_upsert_secret_wrong_nonce_length() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    // Too short
    let result = vault.upsert_secret(&env_id, "KEY", b"ciphertext", &[0u8; 8]);
    assert!(
        matches!(result, Err(DbError::ConstraintViolation(_))),
        "nonce shorter than 12 bytes must return DbError::ConstraintViolation, got: {:?}",
        result
    );

    // Too long
    let result = vault.upsert_secret(&env_id, "KEY", b"ciphertext", &[0u8; 16]);
    assert!(
        matches!(result, Err(DbError::ConstraintViolation(_))),
        "nonce longer than 12 bytes must return DbError::ConstraintViolation, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// (e) delete_secret succeeds
// ---------------------------------------------------------------------------

#[test]
fn test_delete_secret_succeeds() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    vault
        .upsert_secret(&env_id, "TO_DELETE", b"ciphertext", &NONCE_12)
        .expect("upsert must succeed");

    vault
        .delete_secret(&env_id, "TO_DELETE")
        .expect("delete_secret must succeed for an existing secret");

    let result = vault.get_secret(&env_id, "TO_DELETE");
    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_secret after delete must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (f) delete_secret on non-existent key returns DbError::NotFound
// ---------------------------------------------------------------------------

#[test]
fn test_delete_secret_not_found() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    let result = vault.delete_secret(&env_id, "DOES_NOT_EXIST");
    assert!(
        matches!(result, Err(DbError::NotFound)),
        "delete_secret with unknown key must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (g) delete_environment cascades to delete all its secrets
// ---------------------------------------------------------------------------

#[test]
fn test_delete_environment_cascades_to_secrets() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    vault
        .upsert_secret(&env_id, "KEY_A", b"cipher-a", &NONCE_12)
        .expect("upsert KEY_A must succeed");
    vault
        .upsert_secret(&env_id, "KEY_B", b"cipher-b", &NONCE_12)
        .expect("upsert KEY_B must succeed");

    vault
        .delete_environment(&env_id)
        .expect("delete_environment must succeed");

    // Both secrets must be gone via ON DELETE CASCADE
    assert!(
        matches!(vault.get_secret(&env_id, "KEY_A"), Err(DbError::NotFound)),
        "KEY_A must be gone after environment delete"
    );
    assert!(
        matches!(vault.get_secret(&env_id, "KEY_B"), Err(DbError::NotFound)),
        "KEY_B must be gone after environment delete"
    );
}

// ---------------------------------------------------------------------------
// (h) list_secrets returns records ordered by key ASC
// ---------------------------------------------------------------------------

#[test]
fn test_list_secrets_order() {
    let (vault, _tmp) = open_temp_vault();
    let env_id = make_env(&vault);

    // Empty — must return Ok(vec![])
    let empty = vault
        .list_secrets(&env_id)
        .expect("list_secrets on empty environment must return Ok(vec![])");
    assert!(
        empty.is_empty(),
        "list_secrets on empty environment must return []"
    );

    vault
        .upsert_secret(&env_id, "ZEBRA", b"z", &NONCE_12)
        .expect("upsert ZEBRA");
    vault
        .upsert_secret(&env_id, "ALPHA", b"a", &NONCE_12)
        .expect("upsert ALPHA");
    vault
        .upsert_secret(&env_id, "MANGO", b"m", &NONCE_12)
        .expect("upsert MANGO");

    let list = vault
        .list_secrets(&env_id)
        .expect("list_secrets must succeed after inserts");

    assert_eq!(list.len(), 3, "list must contain exactly 3 secrets");

    let keys: Vec<&str> = list.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["ALPHA", "MANGO", "ZEBRA"],
        "secrets must be ordered by key ASC"
    );
}

// ---------------------------------------------------------------------------
// (i) same key name in different environments is independent
// ---------------------------------------------------------------------------

#[test]
fn test_same_key_in_different_environments() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = vault.create_project("my-app").expect("create project");

    let env_dev = vault
        .create_environment(&project_id, "development")
        .expect("create development");
    let env_prod = vault
        .create_environment(&project_id, "production")
        .expect("create production");

    vault
        .upsert_secret(&env_dev, "DATABASE_URL", b"dev-cipher", &NONCE_12)
        .expect("upsert in development");
    vault
        .upsert_secret(&env_prod, "DATABASE_URL", b"prod-cipher", &NONCE_12)
        .expect("upsert in production");

    let dev_record = vault
        .get_secret(&env_dev, "DATABASE_URL")
        .expect("get dev secret");
    let prod_record = vault
        .get_secret(&env_prod, "DATABASE_URL")
        .expect("get prod secret");

    assert_eq!(dev_record.value_encrypted, b"dev-cipher".to_vec());
    assert_eq!(prod_record.value_encrypted, b"prod-cipher".to_vec());
}
