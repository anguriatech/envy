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

use std::io::Write;
use std::process::{Command, Output, Stdio};

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

#[test]
fn bare_invocation_is_help_only_when_stdout_is_piped() {
    let output = Command::new(env!("CARGO_BIN_EXE_envy"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn bare envy");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: envy"));
    assert!(!output.stdout.windows(2).any(|bytes| bytes == [0x1b, b'[']));
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
// Regression — `set --stdin` must not bake in a trailing newline
// ---------------------------------------------------------------------------

/// `echo "secret" | envy set --stdin KEY` (the documented usage) appends a
/// trailing `\n` that is not part of the secret. Verifies the stored value
/// matches the exact `set`/`get` round-trip contract used by
/// `cli_set_and_get_round_trip`, rather than picking up a stray newline that
/// would also silently defeat `envy scan`'s line-based leak matching.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_set_stdin_trims_trailing_newline() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let mut child = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["set", "--stdin", "API_KEY"])
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn envy set --stdin");
    child
        .stdin
        .take()
        .expect("child stdin must be piped")
        .write_all(b"secret123\n")
        .expect("write to child stdin");
    let set_out = child
        .wait_with_output()
        .expect("failed to wait on envy set --stdin");
    assert!(
        set_out.status.success(),
        "envy set --stdin must exit 0, stderr: {}",
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
        "stdin-piped 'secret123\\n' must round-trip to exactly 'secret123' \
         (plus get's own trailing newline), got: {stdout:?}"
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

// ---------------------------------------------------------------------------
// 017 — shell-init prints a static snippet (no keyring needed)
// ---------------------------------------------------------------------------

/// Verifies that `envy shell-init` works everywhere — outside any project and
/// without touching the keyring — since it only prints static text.
#[test]
fn shell_init_prints_snippet_without_project_or_keyring() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = envy(&["shell-init", "bash"], tmp.path());
    assert!(
        out.status.success(),
        "envy shell-init must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("# >>> envy auto-inject (bash) >>>"),
        "snippet must carry its idempotency marker, got: {stdout:?}"
    );
    assert!(
        stdout.contains("__envy_hook"),
        "snippet must define the hook function, got: {stdout:?}"
    );
    assert!(
        stdout.contains("ENVY_AUTO_INJECT"),
        "snippet must document the kill-switch, got: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// 017 — hook outside any project is a silent no-op (no keyring needed)
// ---------------------------------------------------------------------------

/// Verifies the prompt-hook safety contract without a vault: outside a project
/// (and with hook tracking vars cleared for determinism) `envy hook` exits 0
/// with empty stdout, so a bare prompt never breaks.
#[test]
fn hook_outside_project_is_silent_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["hook", "--shell", "bash"])
        .current_dir(tmp.path())
        .env_remove("__ENVY_KEYS")
        .env_remove("__ENVY_ENV")
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy hook");
    assert_eq!(
        out.status.code(),
        Some(0),
        "envy hook must always exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "nothing to unload means empty stdout, got: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---------------------------------------------------------------------------
// 017 — auto flag round-trip via the binary
// ---------------------------------------------------------------------------

/// Verifies `envy auto status|on|off` through the binary: the flag round-trips
/// through `envy.toml`. stdin is nulled so the one-step installer can never
/// block on its `[y/N]` prompt under a TTY; assertions target the flag, not
/// the installer output.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_auto_flag_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let status_out = envy(&["auto", "status"], tmp.path());
    assert!(status_out.status.success());
    assert!(
        String::from_utf8_lossy(&status_out.stdout).contains("off"),
        "fresh project must report auto-inject off"
    );

    let on_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["auto", "on"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn envy auto on");
    assert!(
        on_out.status.success(),
        "envy auto on must exit 0, stderr: {}",
        String::from_utf8_lossy(&on_out.stderr)
    );
    let content = std::fs::read_to_string(tmp.path().join("envy.toml")).expect("read envy.toml");
    assert!(
        content.contains("auto_inject = true"),
        "envy.toml must carry auto_inject = true, got:\n{content}"
    );

    let off_out = envy(&["auto", "off"], tmp.path());
    assert!(off_out.status.success());
    let content = std::fs::read_to_string(tmp.path().join("envy.toml")).expect("read envy.toml");
    assert!(
        content.contains("auto_inject = false"),
        "envy.toml must carry auto_inject = false, got:\n{content}"
    );
}

// ---------------------------------------------------------------------------
// 017 — init --auto-inject never prompts without a TTY
// ---------------------------------------------------------------------------

/// Verifies the one-step setup degrades safely headless: with stdin nulled,
/// `envy init --auto-inject` exits 0, writes the flag, and prints the manual
/// one-liner instead of blocking on the installer prompt.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_init_auto_inject_is_non_interactive_safe() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["init", "--auto-inject"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn envy init");
    assert!(
        out.status.success(),
        "envy init --auto-inject must exit 0 headless, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let content = std::fs::read_to_string(tmp.path().join("envy.toml")).expect("read envy.toml");
    assert!(
        content.contains("auto_inject = true"),
        "envy.toml must carry auto_inject = true, got:\n{content}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("auto-inject enabled."),
        "must confirm the opt-in on stdout"
    );
}

// ---------------------------------------------------------------------------
// 017 — hook injects, unloads, and respects the kill-switch
// ---------------------------------------------------------------------------

/// End-to-end hook behaviour through the binary: inject after `set`, unload
/// previously exported keys, and honour `ENVY_AUTO_INJECT=0`. Hook-related
/// tracking vars are scrubbed from the child environment for determinism.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
fn cli_hook_injects_unloads_and_respects_kill_switch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let on_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["auto", "on"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn envy auto on");
    assert!(on_out.status.success());

    envy(&["set", "HOOK_TEST_VAR=hook_hello"], tmp.path());

    // Inject: exports for the vault secret plus tracking state.
    let hook_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["hook", "--shell", "bash"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .env_remove("__ENVY_KEYS")
        .env_remove("__ENVY_ENV")
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy hook");
    assert_eq!(hook_out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&hook_out.stdout);
    assert!(
        stdout.contains("export HOOK_TEST_VAR='hook_hello';"),
        "hook must export the vault secret, got: {stdout:?}"
    );
    assert!(
        stdout.contains("__ENVY_KEYS"),
        "hook must maintain tracking state, got: {stdout:?}"
    );

    // Kill-switch: unload previously exported keys, never export values —
    // even for keys the parent shell claims envy exported before.
    let killed_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["hook", "--shell", "bash"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .env("__ENVY_KEYS", "HOOK_TEST_VAR")
        .env("ENVY_AUTO_INJECT", "0")
        .env_remove("__ENVY_ENV")
        .env_remove("ENVY_ENV")
        .output()
        .expect("failed to spawn envy hook");
    assert_eq!(killed_out.status.code(), Some(0));
    let killed = String::from_utf8_lossy(&killed_out.stdout);
    assert!(
        !killed.contains("export HOOK_TEST_VAR="),
        "kill-switch must never export values, got: {killed:?}"
    );
    assert!(
        killed.contains("unset HOOK_TEST_VAR;"),
        "kill-switch must unload stale keys, got: {killed:?}"
    );

    // Outside any project: silent, exit 0.
    let bare = tempfile::tempdir().expect("tempdir");
    let outside_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["hook", "--shell", "bash"])
        .current_dir(bare.path())
        .stdin(Stdio::null())
        .env_remove("__ENVY_KEYS")
        .env_remove("__ENVY_ENV")
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy hook");
    assert_eq!(outside_out.status.code(), Some(0));
    assert!(
        outside_out.stdout.is_empty(),
        "nothing to unload means empty stdout, got: {:?}",
        String::from_utf8_lossy(&outside_out.stdout)
    );
}
