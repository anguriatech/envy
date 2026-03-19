// Integration tests for the db layer — plaintext absence and SQLCipher encryption verification.

use envy::db::Vault;

const DUMMY_KEY: [u8; 32] = [0u8; 32];

// ---------------------------------------------------------------------------
// Defense-in-depth: SQLCipher full-file encryption hides stored bytes
// ---------------------------------------------------------------------------

/// Verifies that a known byte sentinel written via `upsert_secret` does NOT
/// appear anywhere in the raw vault file bytes after the vault is closed.
///
/// This test validates that:
/// 1. The `bundled-sqlcipher` feature is correctly enabled (full-file AES-256).
/// 2. The ciphertext blob stored by the DB layer is opaque after encryption.
///
/// Note: in real usage `value_encrypted` would be AES-256-GCM ciphertext
/// produced by the crypto layer. Here we use a detectable sentinel pattern
/// to prove the file-level encryption is active — if SQLCipher were absent,
/// this assertion would fail because SQLite stores blobs verbatim.
#[test]
fn test_sentinel_not_visible_in_raw_vault_file() {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault_path = tmp.path().to_path_buf();

    // The byte pattern we expect SQLCipher to hide. In real use this would be
    // AES-256-GCM ciphertext, never plaintext — but the test needs a detectable
    // pattern to assert absence.
    let sentinel: &[u8] = b"SENTINEL_PLAINTEXT_12345";
    let nonce = [0xAB; 12];

    {
        let vault = Vault::open(&vault_path, &DUMMY_KEY)
            .expect("Vault::open must succeed on a fresh temp file");

        let project_id = vault
            .create_project("security-test")
            .expect("create_project must succeed");
        let env_id = vault
            .create_environment(&project_id, "test")
            .expect("create_environment must succeed");

        vault
            .upsert_secret(&env_id, "SENTINEL_KEY", sentinel, &nonce)
            .expect("upsert_secret must succeed");

        // Explicitly close to flush and checkpoint the WAL file before reading
        // raw bytes. Without this, data may still be in the WAL rather than
        // the main .db file, causing a false-positive pass.
        vault.close().expect("Vault::close must succeed");
    }

    let file_bytes =
        std::fs::read(&vault_path).expect("reading the vault file must succeed after close");

    assert!(
        !file_bytes.windows(sentinel.len()).any(|w| w == sentinel),
        "sentinel byte pattern must NOT appear in the raw vault file — \
         SQLCipher full-file encryption should make it undetectable. \
         If this assertion fails, the bundled-sqlcipher feature may not be active."
    );
}

// ---------------------------------------------------------------------------
// SQLCipher version is non-empty (confirms bundled-sqlcipher is active)
// ---------------------------------------------------------------------------

/// Verifies that `PRAGMA cipher_version` returns a non-empty string.
/// This is only available when the `bundled-sqlcipher` feature is enabled.
#[test]
fn test_cipher_version_is_available() {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault = Vault::open(tmp.path(), &DUMMY_KEY).expect("Vault::open must succeed");

    let version = vault
        .pragma_str("cipher_version")
        .expect("PRAGMA cipher_version must return a value when bundled-sqlcipher is active");

    assert!(
        !version.is_empty(),
        "cipher_version must be non-empty — got an empty string, \
         which suggests bundled-sqlcipher is not active"
    );
}
