//! End-to-end integration tests for the `envy.enc` sync artifact pipeline.
//!
//! Each test uses `tempfile::TempDir` for complete isolation — no shared state
//! between tests. No OS keyring is required: all vaults are opened with a raw
//! `[u8; 32]` test key.

use std::collections::BTreeMap;

use base64ct::{Base64, Encoding};
use zeroize::Zeroizing;

use envy::core::{read_artifact, seal_artifact, set_secret, unseal_artifact, write_artifact};
use envy::crypto::artifact::{ARTIFACT_VERSION, ArtifactPayload, SyncArtifact, seal_envelope};
use envy::db::{ProjectId, Vault};

// ---------------------------------------------------------------------------
// T033 — Shared test vault helper
// ---------------------------------------------------------------------------

const TEST_KEY: [u8; 32] = [9u8; 32];

/// Opens a fresh, isolated vault in `tmp`, creates a project, and returns the
/// vault handle and project id.  Mirrors the pattern used in `tests/cli_integration.rs`.
fn setup_test_vault(tmp: &tempfile::TempDir) -> (Vault, ProjectId) {
    let path = tmp.path().join("vault.db");
    let vault = Vault::open(&path, &TEST_KEY).expect("vault must open");
    let pid = vault
        .create_project("integration-test")
        .expect("project must be created");
    (vault, pid)
}

// ---------------------------------------------------------------------------
// T034 — e2e_seal_and_unseal_full_vault
// ---------------------------------------------------------------------------

/// Sealing then unsealing with the same passphrase recovers all secrets byte-for-byte.
#[test]
fn e2e_seal_and_unseal_full_vault() {
    let tmp = tempfile::tempdir().expect("tempdir must succeed");
    let (vault, pid) = setup_test_vault(&tmp);

    set_secret(
        &vault,
        &TEST_KEY,
        &pid,
        "development",
        "STRIPE_KEY",
        "sk_test_abc",
    )
    .expect("set_secret must succeed");
    set_secret(
        &vault,
        &TEST_KEY,
        &pid,
        "development",
        "DATABASE_URL",
        "postgres://dev",
    )
    .expect("set_secret must succeed");
    set_secret(
        &vault,
        &TEST_KEY,
        &pid,
        "production",
        "STRIPE_KEY",
        "sk_live_xyz",
    )
    .expect("set_secret must succeed");

    let passphrase = "correct-horse-battery-staple";
    let artifact = seal_artifact(
        &vault,
        &TEST_KEY,
        &pid,
        passphrase,
        Some(&["development", "production"]),
    )
    .expect("seal_artifact must succeed");

    assert_eq!(artifact.version, ARTIFACT_VERSION);
    assert!(artifact.environments.contains_key("development"));
    assert!(artifact.environments.contains_key("production"));

    let result = unseal_artifact(&artifact, passphrase).expect("unseal_artifact must succeed");

    assert!(
        result.skipped.is_empty(),
        "no environments must be skipped with the correct passphrase"
    );
    assert_eq!(
        result.imported.len(),
        2,
        "both environments must be imported"
    );

    let dev = &result.imported["development"];
    assert_eq!(dev["STRIPE_KEY"].as_str(), "sk_test_abc");
    assert_eq!(dev["DATABASE_URL"].as_str(), "postgres://dev");

    let prod = &result.imported["production"];
    assert_eq!(prod["STRIPE_KEY"].as_str(), "sk_live_xyz");
}

// ---------------------------------------------------------------------------
// T035 — e2e_wrong_passphrase_skips_all_environments
// ---------------------------------------------------------------------------

/// Unsealing with a wrong passphrase skips ALL environments and returns no hard error.
#[test]
fn e2e_wrong_passphrase_skips_all_environments() {
    let tmp = tempfile::tempdir().expect("tempdir must succeed");
    let (vault, pid) = setup_test_vault(&tmp);

    set_secret(&vault, &TEST_KEY, &pid, "development", "KEY", "value")
        .expect("set_secret must succeed");

    let artifact = seal_artifact(&vault, &TEST_KEY, &pid, "correct", Some(&["development"]))
        .expect("seal_artifact must succeed");

    let result = unseal_artifact(&artifact, "wrong-passphrase")
        .expect("unseal_artifact must return Ok even when all envs are skipped");

    assert!(
        result.imported.is_empty(),
        "nothing must be imported with wrong passphrase"
    );
    assert_eq!(
        result.skipped,
        vec!["development"],
        "development must be in skipped list"
    );
}

// ---------------------------------------------------------------------------
// T036 — e2e_partial_access_progressive_disclosure
// ---------------------------------------------------------------------------

/// A developer with the dev key imports development; production is gracefully skipped.
#[test]
fn e2e_partial_access_progressive_disclosure() {
    // Build the artifact manually: dev env with "dev-key", prod env with "prod-key".
    let mut dev_secrets = BTreeMap::new();
    dev_secrets.insert("API_KEY".to_string(), Zeroizing::new("dev-api".to_string()));
    let dev_payload = ArtifactPayload {
        secrets: dev_secrets,
    };

    let mut prod_secrets = BTreeMap::new();
    prod_secrets.insert("DB_URL".to_string(), Zeroizing::new("prod-db".to_string()));
    let prod_payload = ArtifactPayload {
        secrets: prod_secrets,
    };

    let dev_envelope =
        seal_envelope("dev-key", &dev_payload).expect("seal dev envelope must succeed");
    let prod_envelope =
        seal_envelope("prod-key", &prod_payload).expect("seal prod envelope must succeed");

    let mut environments = BTreeMap::new();
    environments.insert("development".to_string(), dev_envelope);
    environments.insert("production".to_string(), prod_envelope);

    let artifact = SyncArtifact {
        version: ARTIFACT_VERSION,
        environments,
    };

    // Unseal with the dev key only.
    let result =
        unseal_artifact(&artifact, "dev-key").expect("unseal must succeed with partial access");

    // development must be imported.
    assert!(
        result.imported.contains_key("development"),
        "development must be imported"
    );
    assert_eq!(
        result.imported["development"]["API_KEY"].as_str(),
        "dev-api"
    );

    // production must be gracefully skipped — not an error.
    assert!(
        result.skipped.contains(&"production".to_string()),
        "production must be skipped"
    );
    assert_eq!(
        result.imported.len(),
        1,
        "only development must be imported"
    );
    assert_eq!(result.skipped.len(), 1, "only production must be skipped");
}

// ---------------------------------------------------------------------------
// T037 — e2e_tampered_ciphertext_skips_environment
// ---------------------------------------------------------------------------

/// A single-byte ciphertext flip causes authentication failure → env is skipped, vault untouched.
#[test]
fn e2e_tampered_ciphertext_skips_environment() {
    let tmp = tempfile::tempdir().expect("tempdir must succeed");
    let (vault, pid) = setup_test_vault(&tmp);

    set_secret(&vault, &TEST_KEY, &pid, "development", "SECRET", "value")
        .expect("set_secret must succeed");

    let mut artifact = seal_artifact(&vault, &TEST_KEY, &pid, "pass", Some(&["development"]))
        .expect("seal_artifact must succeed");

    // Tamper: decode ciphertext, flip first byte, re-encode.
    let envelope = artifact
        .environments
        .get_mut("development")
        .expect("env must exist");
    let mut ct_bytes =
        Base64::decode_vec(&envelope.ciphertext).expect("ciphertext must be valid Base64");
    ct_bytes[0] ^= 0xFF;
    envelope.ciphertext = Base64::encode_string(&ct_bytes);

    let result = unseal_artifact(&artifact, "pass")
        .expect("unseal must return Ok even for tampered envelope");

    assert!(
        result.imported.is_empty(),
        "tampered environment must not be imported"
    );
    assert!(
        result.skipped.contains(&"development".to_string()),
        "tampered env must be in skipped"
    );
}

// ---------------------------------------------------------------------------
// T038 — e2e_write_read_artifact_file_round_trip
// ---------------------------------------------------------------------------

/// write_artifact + read_artifact preserves the full artifact structure on disk.
#[test]
fn e2e_write_read_artifact_file_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir must succeed");
    let (vault, pid) = setup_test_vault(&tmp);
    let artifact_path = tmp.path().join("envy.enc");

    set_secret(&vault, &TEST_KEY, &pid, "development", "KEY_A", "val_a")
        .expect("set_secret must succeed");
    set_secret(&vault, &TEST_KEY, &pid, "staging", "KEY_B", "val_b")
        .expect("set_secret must succeed");

    let artifact = seal_artifact(
        &vault,
        &TEST_KEY,
        &pid,
        "file-round-trip-pass",
        Some(&["development", "staging"]),
    )
    .expect("seal_artifact must succeed");

    write_artifact(&artifact, &artifact_path).expect("write_artifact must succeed");

    // Verify the file exists and is valid JSON.
    assert!(artifact_path.exists(), "envy.enc must exist after write");
    let raw = std::fs::read_to_string(&artifact_path).expect("must be able to read envy.enc");
    assert!(
        raw.contains("\"version\""),
        "JSON must contain version field"
    );
    assert!(
        raw.contains("\"development\""),
        "JSON must contain development key"
    );
    assert!(raw.contains("\"staging\""), "JSON must contain staging key");
    // Key names must NOT appear in plaintext.
    assert!(
        !raw.contains("KEY_A"),
        "secret key names must not appear in plaintext JSON"
    );
    assert!(
        !raw.contains("val_a"),
        "secret values must not appear in plaintext JSON"
    );

    // Round-trip: read back and verify structure.
    let recovered = read_artifact(&artifact_path).expect("read_artifact must succeed");
    assert_eq!(recovered.version, ARTIFACT_VERSION);
    assert!(recovered.environments.contains_key("development"));
    assert!(recovered.environments.contains_key("staging"));
    assert_eq!(recovered.environments.len(), 2);
}
