// Integration tests for the db layer — schema creation, migration, idempotency.

use envy::db::{DbError, Vault};

/// A 32-byte all-zero key used across schema tests.
/// Key strength is irrelevant here — we are testing schema structure, not cryptography.
const DUMMY_KEY: [u8; 32] = [0u8; 32];

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Opens a fresh vault backed by a temporary file.
/// The temp file is deleted automatically when `_guard` is dropped.
fn open_temp_vault() -> (Vault, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new()
        // SAFETY: creating a temp file in the OS temp directory is always
        // expected to succeed in a normal test environment.
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault = Vault::open(tmp.path(), &DUMMY_KEY)
        .expect("Vault::open on a fresh temp file always succeeds with a valid key");
    (vault, tmp)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// (a) Vault::open succeeds with a dummy 32-byte key.
///
/// This validates that SQLCipher is active, the PRAGMA key was accepted, and
/// the migration runner completed without errors.
#[test]
fn test_vault_opens_successfully() {
    let (_vault, _tmp) = open_temp_vault();
    // Reaching this line means open succeeded — the assertion is implicit.
}

/// (b) PRAGMA user_version equals 2 after the first open.
///
/// Verifies that `run_migrations` ran both V1 and V2 migrations and set the
/// schema version to 2.
#[test]
fn test_schema_version_is_one_after_open() {
    let (vault, _tmp) = open_temp_vault();
    let version = vault
        .pragma_int("user_version")
        .expect("pragma_int('user_version') must succeed on an open vault");
    assert_eq!(version, 2, "user_version must be 2 after initial migration");
}

/// (c) All three tables exist in the vault after the first open.
///
/// Verifies that the CREATE TABLE statements in SCHEMA_V1 were executed.
#[test]
fn test_all_three_tables_exist() {
    let (vault, _tmp) = open_temp_vault();

    for table in &["projects", "environments", "secrets"] {
        let exists = vault
            .table_exists(table)
            .unwrap_or_else(|e| panic!("table_exists('{}') failed: {}", table, e));
        assert!(exists, "table '{}' must exist after migration", table);
    }
}

/// (d) Opening the same vault file a second time is idempotent.
///
/// Verifies that `CREATE TABLE IF NOT EXISTS` and the user_version guard
/// prevent any error or duplicate creation on re-open.
#[test]
fn test_reopen_is_idempotent() {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");

    // First open — runs migrations, sets user_version = 2.
    {
        let vault = Vault::open(tmp.path(), &DUMMY_KEY).expect("first open must succeed");
        vault.close().expect("close must succeed");
    }

    // Second open — should succeed without error and leave user_version = 2.
    let vault =
        Vault::open(tmp.path(), &DUMMY_KEY).expect("second open of the same vault must succeed");
    let version = vault
        .pragma_int("user_version")
        .expect("pragma_int must succeed on second open");
    assert_eq!(version, 2, "user_version must still be 2 on re-open");
}

/// (e) PRAGMA foreign_keys is ON (value = 1) after open.
///
/// Verifies that Vault::open sets `PRAGMA foreign_keys = ON`.
/// Without this, ON DELETE CASCADE and FK constraints are silently ignored.
#[test]
fn test_foreign_keys_pragma_is_on() {
    let (vault, _tmp) = open_temp_vault();
    let fk = vault
        .pragma_int("foreign_keys")
        .expect("pragma_int('foreign_keys') must succeed");
    assert_eq!(fk, 1, "foreign_keys must be ON (1) after Vault::open");
}

/// (f) PRAGMA journal_mode is WAL after open.
///
/// Verifies that Vault::open switches the database to Write-Ahead Logging mode,
/// which enables concurrent reads during writes.
#[test]
fn test_journal_mode_is_wal() {
    let (vault, _tmp) = open_temp_vault();
    let mode = vault
        .pragma_str("journal_mode")
        .expect("pragma_str('journal_mode') must succeed");
    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "journal_mode must be WAL after Vault::open"
    );
}

/// Wrong key on an existing vault returns DbError::EncryptionError.
///
/// This guards Principle I: if the master key is wrong, the vault must reject
/// access rather than returning garbage or panicking.
#[test]
fn test_wrong_key_returns_encryption_error() {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");

    // Create and seal the vault with the dummy key.
    {
        let vault = Vault::open(tmp.path(), &DUMMY_KEY).expect("first open must succeed");
        vault.close().expect("close must succeed");
    }

    // Re-open with a different key — must fail with EncryptionError.
    let wrong_key = [0xFFu8; 32];
    let result = Vault::open(tmp.path(), &wrong_key);
    assert!(
        matches!(result, Err(DbError::EncryptionError)),
        "opening with the wrong key must return DbError::EncryptionError, got: {:?}",
        result
    );
}
