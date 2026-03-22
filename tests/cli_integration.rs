//! End-to-end CLI integration tests — see specs/004-cli-interface/tasks.md §Phase 5
//!
//! # Keyring requirement
//! All tests in this file invoke `envy init`, which calls
//! `crypto::get_or_create_master_key` via the OS credential store (Linux:
//! Secret Service / libsecret, macOS: Keychain, Windows: Credential Manager).
//! In headless CI environments without a keyring daemon, every test is
//! annotated with `#[ignore]` so the suite does not fail.
//!
//! To run these tests manually in an environment with a live keyring:
//!   cargo test -- --ignored
//!   cargo test --test cli_integration -- --ignored

use std::process::{Command, Output};

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

/// Initialises an Envy project in `dir` by running `envy init`.
///
/// Panics if the process cannot be spawned — that is always a test-environment
/// problem, not a test failure.
fn setup_project(dir: &std::path::Path) {
    Command::new(env!("CARGO_BIN_EXE_envy"))
        .arg("init")
        .current_dir(dir)
        .status()
        .expect("envy init failed to spawn");
}

/// Runs `envy` with the given args in the given working directory and returns
/// the full `Output` (status, stdout, stderr).
fn envy(args: &[&str], cwd: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to spawn envy")
}

// ---------------------------------------------------------------------------
// T028 — US1: init creates manifest
// ---------------------------------------------------------------------------

/// Verifies that `envy init` exits 0 and writes a valid `envy.toml` containing
/// a UUID-formatted `project_id` field.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_init_creates_manifest() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let status = Command::new(env!("CARGO_BIN_EXE_envy"))
        .arg("init")
        .current_dir(tmp.path())
        .status()
        .expect("envy init failed to spawn");

    assert!(status.success(), "envy init must exit 0, got: {status}");

    let manifest_path = tmp.path().join("envy.toml");
    assert!(manifest_path.exists(), "envy.toml must be created by init");

    let content = std::fs::read_to_string(&manifest_path).expect("read envy.toml");
    // Must contain a project_id = "..." line with a UUID-like value.
    assert!(
        content.contains("project_id"),
        "envy.toml must contain a project_id field, got:\n{content}"
    );
    // Extract the UUID and check it looks right (8-4-4-4-12 hyphenated hex).
    let uuid_line = content
        .lines()
        .find(|l| l.contains("project_id"))
        .expect("project_id line must exist");
    // UUID pattern: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (36 chars)
    let uuid = uuid_line
        .split('"')
        .nth(1)
        .expect("project_id value must be quoted in TOML");
    assert_eq!(
        uuid.len(),
        36,
        "project_id must be a 36-char UUID, got: {uuid}"
    );
    assert_eq!(
        uuid.chars().filter(|&c| c == '-').count(),
        4,
        "project_id UUID must contain exactly 4 hyphens, got: {uuid}"
    );
}

// ---------------------------------------------------------------------------
// T029 — US2: set/get round-trip
// ---------------------------------------------------------------------------

/// Verifies that a value stored with `envy set` is returned verbatim by
/// `envy get` with no extra labels or whitespace — satisfying the UNIX-pipeline
/// stdout contract (`{value}\n` only).
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_set_and_get_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let set_out = envy(&["set", "API_KEY=secret123"], tmp.path());
    assert!(
        set_out.status.success(),
        "envy set must exit 0, stderr: {}",
        String::from_utf8_lossy(&set_out.stderr)
    );

    let get_out = envy(&["get", "API_KEY"], tmp.path());
    assert!(
        get_out.status.success(),
        "envy get must exit 0, stderr: {}",
        String::from_utf8_lossy(&get_out.stderr)
    );

    let stdout = String::from_utf8_lossy(&get_out.stdout);
    assert_eq!(
        stdout.as_ref(),
        "secret123\n",
        "stdout must be exactly 'secret123\\n', got: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// T030 — US2: list never shows values
// ---------------------------------------------------------------------------

/// Verifies that `envy list` prints key names to stdout and NEVER prints secret
/// values — a hard security invariant.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_list_never_shows_values() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    envy(&["set", "API_KEY=secret123"], tmp.path());

    let list_out = envy(&["list"], tmp.path());
    assert!(
        list_out.status.success(),
        "envy list must exit 0, stderr: {}",
        String::from_utf8_lossy(&list_out.stderr)
    );

    let stdout = String::from_utf8_lossy(&list_out.stdout);
    assert!(
        stdout.contains("API_KEY"),
        "stdout must contain the key name 'API_KEY', got: {stdout:?}"
    );
    assert!(
        !stdout.contains("secret123"),
        "stdout must NOT contain the secret value, got: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// T031 — US2: rm then get fails
// ---------------------------------------------------------------------------

/// Verifies that `envy rm` deletes a secret and that a subsequent `envy get`
/// exits with a non-zero code (not-found).
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_rm_then_get_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    envy(&["set", "DEL_KEY=val"], tmp.path());

    let rm_out = envy(&["rm", "DEL_KEY"], tmp.path());
    assert!(
        rm_out.status.success(),
        "envy rm must exit 0, stderr: {}",
        String::from_utf8_lossy(&rm_out.stderr)
    );

    let get_out = envy(&["get", "DEL_KEY"], tmp.path());
    assert!(
        !get_out.status.success(),
        "envy get after rm must exit non-zero (secret was deleted)"
    );
}

// ---------------------------------------------------------------------------
// T032 — US3: run injects secrets
// ---------------------------------------------------------------------------

/// Verifies that `envy run` injects the project's secrets as environment
/// variables into the child process.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_run_injects_secrets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    envy(&["set", "ENVY_TEST_VAR=hello"], tmp.path());

    let run_out = envy(&["run", "--", "printenv", "ENVY_TEST_VAR"], tmp.path());
    assert!(
        run_out.status.success(),
        "envy run must exit 0, stderr: {}",
        String::from_utf8_lossy(&run_out.stderr)
    );

    let stdout = String::from_utf8_lossy(&run_out.stdout);
    assert_eq!(
        stdout.as_ref(),
        "hello\n",
        "stdout must be exactly 'hello\\n', got: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// T033 — US3: run proxies exit code
// ---------------------------------------------------------------------------

/// Verifies that `envy run` forwards the child's exit code exactly.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_run_proxies_exit_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let run_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["run", "--", "sh", "-c", "exit 42"])
        .current_dir(tmp.path())
        .status()
        .expect("failed to spawn envy run");

    assert_eq!(
        run_out.code(),
        Some(42),
        "envy run must proxy the child exit code 42 exactly"
    );
}

// ---------------------------------------------------------------------------
// T033 (006) — encrypt and enc alias work
// ---------------------------------------------------------------------------

/// Verifies that `envy encrypt` and its alias `envy enc` both exit 0 and
/// produce `envy.enc` when `ENVY_PASSPHRASE` is set in the environment.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_encrypt_and_enc_alias_work() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    // Seed one secret so the sealed artifact is non-trivial.
    envy(&["set", "ENCRYPT_TEST=hello"], tmp.path());

    // Run via full command name.
    let enc_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["encrypt"])
        .env("ENVY_PASSPHRASE", "integration-pass")
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy encrypt");

    assert!(
        enc_out.status.success(),
        "envy encrypt must exit 0, stderr: {}",
        String::from_utf8_lossy(&enc_out.stderr)
    );
    assert!(
        tmp.path().join("envy.enc").exists(),
        "envy encrypt must create envy.enc"
    );

    // Remove the artifact and run via alias to confirm the alias is wired correctly.
    std::fs::remove_file(tmp.path().join("envy.enc")).expect("remove envy.enc");

    let alias_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["enc"])
        .env("ENVY_PASSPHRASE", "integration-pass")
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy enc");

    assert!(
        alias_out.status.success(),
        "envy enc alias must exit 0, stderr: {}",
        String::from_utf8_lossy(&alias_out.stderr)
    );
    assert!(
        tmp.path().join("envy.enc").exists(),
        "envy enc alias must create envy.enc"
    );
}

// ---------------------------------------------------------------------------
// T034 (006) — decrypt and dec alias work
// ---------------------------------------------------------------------------

/// Verifies that `envy decrypt` and its alias `envy dec` both exit 0 and
/// upsert secrets into the vault when a valid `envy.enc` is present.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_decrypt_and_dec_alias_work() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    // Seed a secret and seal it.
    envy(&["set", "DECRYPT_TEST=world"], tmp.path());
    let seal_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["encrypt"])
        .env("ENVY_PASSPHRASE", "dec-test-pass")
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy encrypt");
    assert!(
        seal_out.status.success(),
        "encrypt setup must succeed, stderr: {}",
        String::from_utf8_lossy(&seal_out.stderr)
    );

    // Run decrypt via full command name.
    let dec_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["decrypt"])
        .env("ENVY_PASSPHRASE", "dec-test-pass")
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy decrypt");

    assert!(
        dec_out.status.success(),
        "envy decrypt must exit 0, stderr: {}",
        String::from_utf8_lossy(&dec_out.stderr)
    );

    // Confirm the alias is also wired correctly.
    let alias_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["dec"])
        .env("ENVY_PASSPHRASE", "dec-test-pass")
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy dec");

    assert!(
        alias_out.status.success(),
        "envy dec alias must exit 0, stderr: {}",
        String::from_utf8_lossy(&alias_out.stderr)
    );
}

// ---------------------------------------------------------------------------
// T034 — US4: migrate imports .env file
// ---------------------------------------------------------------------------

/// Verifies that `envy migrate` reads a `.env` file, imports all valid
/// `KEY=VALUE` pairs, skips comments and blank lines, and that each imported
/// secret is retrievable via `envy get`.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_migrate_imports_env_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    // Write a .env file with 3 valid pairs, 1 comment, and 1 blank line.
    let env_file = tmp.path().join("legacy.env");
    std::fs::write(
        &env_file,
        "# This is a comment\nDB_HOST=localhost\nDB_PORT=5432\n\nDB_NAME=myapp\n",
    )
    .expect("write legacy.env");

    let migrate_out = envy(
        &[
            "migrate",
            env_file.to_str().expect("env file path is UTF-8"),
        ],
        tmp.path(),
    );
    assert!(
        migrate_out.status.success(),
        "envy migrate must exit 0, stderr: {}",
        String::from_utf8_lossy(&migrate_out.stderr)
    );

    // Verify all 3 keys are retrievable with correct values.
    for (key, expected) in [
        ("DB_HOST", "localhost"),
        ("DB_PORT", "5432"),
        ("DB_NAME", "myapp"),
    ] {
        let get_out = envy(&["get", key], tmp.path());
        assert!(
            get_out.status.success(),
            "envy get {key} must succeed after migrate, stderr: {}",
            String::from_utf8_lossy(&get_out.stderr)
        );
        let stdout = String::from_utf8_lossy(&get_out.stdout);
        assert_eq!(
            stdout.as_ref(),
            format!("{expected}\n").as_str(),
            "envy get {key} must return '{expected}\\n', got: {stdout:?}"
        );
    }
}
