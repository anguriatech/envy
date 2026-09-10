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
// 018 — shell-init prints a static PATH line (no keyring needed)
// ---------------------------------------------------------------------------

/// Verifies that `envy shell-init` works everywhere — outside any project and
/// without touching the keyring — since it only prints a static `PATH` line.
#[test]
fn shell_init_prints_path_line_without_project_or_keyring() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = envy(&["shell-init", "bash"], tmp.path());
    assert!(
        out.status.success(),
        "envy shell-init must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("$HOME/.envy/shims:$PATH"),
        "bash snippet must prepend the shims dir, got: {stdout:?}"
    );
    assert!(
        stdout.contains("envy doctor"),
        "snippet must point at doctor, got: {stdout:?}"
    );
    assert!(
        !stdout.contains("__envy_hook"),
        "no hook machinery may remain, got: {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// 018 — auto flag round-trip via the binary
// ---------------------------------------------------------------------------

/// Verifies `envy auto status|on|off` through the binary: the flag round-trips
/// through `envy.toml`. stdin is nulled for determinism under a TTY.
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
// 018 — init --auto-inject writes the flag without side effects
// ---------------------------------------------------------------------------

/// Verifies `envy init --auto-inject` exits 0, writes the flag, and only
/// prints next steps (nothing is installed or generated automatically).
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
// 018 — reshim generates and prunes without vault or keyring
// ---------------------------------------------------------------------------

/// Hermetic: HOME is redirected (shims land in a temp dir) and the manifest
/// is hand-written, so no keyring, vault, or home pollution occurs. `reshim`
/// needs a manifest but deliberately never opens the vault.
#[test]
fn reshim_generates_and_prunes_hermetically() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).expect("mkdir home");
    std::fs::write(
        tmp.path().join("envy.toml"),
        "project_id = \"00000000-0000-4000-8000-000000000001\"\nauto_inject = true\n",
    )
    .expect("write envy.toml");
    std::fs::write(tmp.path().join("package.json"), "{}").expect("write package.json");

    let shim_path = |name: &str| -> std::path::PathBuf {
        let dir = tmp.path().join("home").join(".envy").join("shims");
        if cfg!(windows) {
            dir.join(format!("{name}.cmd"))
        } else {
            dir.join(name)
        }
    };
    let wanted = ["npm", "npx", "node"];
    let pre: Vec<bool> = wanted.iter().map(|&n| shim_path(n).exists()).collect();

    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["reshim"])
        .current_dir(tmp.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .expect("failed to spawn envy reshim");
    assert!(
        out.status.success(),
        "envy reshim must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for (&name, &was_there) in wanted.iter().zip(pre.iter()) {
        let path = shim_path(name);
        assert!(path.is_file(), "shim for {name} must exist");
        if !was_there {
            let content = std::fs::read_to_string(&path).expect("read shim");
            assert!(
                content.contains("envy-shim-source: auto"),
                "shim for {name} must carry provenance"
            );
            assert!(
                content.contains("envy exec --"),
                "shim for {name} must delegate to exec"
            );
        }
    }

    // Prune after removing the trigger: auto shims go (unless pre-existing).
    std::fs::remove_file(tmp.path().join("package.json")).expect("remove package.json");
    let prune_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["reshim", "--prune"])
        .current_dir(tmp.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .expect("failed to spawn envy reshim --prune");
    assert!(prune_out.status.success());
    for (&name, &was_there) in wanted.iter().zip(pre.iter()) {
        if !was_there {
            assert!(
                !shim_path(name).exists(),
                "prune must remove auto shim {name}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 018 — shim add/rm/list is hermetic
// ---------------------------------------------------------------------------

/// Manual shim management through the binary with HOME redirected (plus a
/// unique name and end-of-test removal, so even a HOME override miss on an
/// exotic platform leaves no litter).
#[test]
fn shim_add_rm_list_is_hermetic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let run = |args: &[&str]| -> Output {
        Command::new(env!("CARGO_BIN_EXE_envy"))
            .args(args)
            .current_dir(tmp.path())
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .output()
            .expect("failed to spawn envy")
    };

    let bad = run(&["shim", "add", "a/b"]);
    assert_eq!(
        bad.status.code(),
        Some(2),
        "invalid shim name must exit 2, stderr: {}",
        String::from_utf8_lossy(&bad.stderr)
    );

    let add = run(&["shim", "add", "envy-e2e-probe"]);
    assert!(
        add.status.success(),
        "shim add must exit 0, stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let list = run(&["shim", "list"]);
    assert!(
        String::from_utf8_lossy(&list.stdout).contains("envy-e2e-probe"),
        "shim list must show the added shim"
    );
    let rm = run(&["shim", "rm", "envy-e2e-probe"]);
    assert!(
        rm.status.success(),
        "shim rm must exit 0, stderr: {}",
        String::from_utf8_lossy(&rm.stderr)
    );
    let list2 = run(&["shim", "list"]);
    assert!(
        !String::from_utf8_lossy(&list2.stdout).contains("envy-e2e-probe"),
        "shim list must not show the removed shim"
    );
    let rm_missing = run(&["shim", "rm", "envy-e2e-probe"]);
    assert_eq!(
        rm_missing.status.code(),
        Some(1),
        "rm of a missing shim must exit 1"
    );
}

// ---------------------------------------------------------------------------
// 018 — exec outside a project proxies without vault or keyring
// ---------------------------------------------------------------------------

/// Runs the binary through itself: the outer `exec` takes the fast path (no
/// manifest → direct spawn, zero vault access) and must proxy the inner exit
/// code exactly. No shell, no keyring, all OSes.
#[test]
fn exec_outside_project_proxies_without_vault() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let me = env!("CARGO_BIN_EXE_envy");
    // Inner clap parse error → exit 2, proving exact proxying.
    let out = Command::new(me)
        .args(["exec", "--", me, "--definitely-not-a-subcommand"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy exec");
    assert_eq!(
        out.status.code(),
        Some(2),
        "exec must proxy the child exit code"
    );
    // Inner success → exit 0 (`completions` needs no vault or manifest).
    let ok = Command::new(me)
        .args(["exec", "--", me, "completions", "bash"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy exec");
    assert!(
        ok.status.success(),
        "exec must proxy success, stderr: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

// ---------------------------------------------------------------------------
// 018 — doctor findings without vault or keyring
// ---------------------------------------------------------------------------

/// `doctor` inspects `PATH` and the manifest flag only: with shims absent from
/// a controlled `PATH`, it exits 1 naming the problem — on every OS.
#[test]
fn doctor_flags_shims_missing_from_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["doctor"])
        .current_dir(tmp.path())
        .env("PATH", tmp.path().join("empty-bin"))
        .output()
        .expect("failed to spawn envy doctor");
    assert_eq!(
        out.status.code(),
        Some(1),
        "doctor must exit 1 with findings"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("not on PATH"),
        "doctor must name the problem"
    );
}

/// Inside an opted-in project (hand-written manifest, no vault) with shims
/// missing, `doctor` reports coverage findings instead of failing.
#[test]
fn doctor_reports_missing_project_shims() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("envy.toml"),
        "project_id = \"00000000-0000-4000-8000-000000000002\"\nauto_inject = true\n",
    )
    .expect("write envy.toml");
    std::fs::write(tmp.path().join("package.json"), "{}").expect("write package.json");
    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["doctor"])
        .current_dir(tmp.path())
        .env("PATH", tmp.path().join("empty-bin"))
        .output()
        .expect("failed to spawn envy doctor");
    assert_eq!(
        out.status.code(),
        Some(1),
        "doctor must exit 1 with findings"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("shims"),
        "doctor must mention shims"
    );
}

/// Full green cycle on Unix (HOME redirect is certain there): shims present
/// first on a controlled `PATH`, no project — exit 0, all checks passed.
#[test]
#[cfg(unix)]
fn doctor_passes_with_shims_first_and_no_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let shims = home.join(".envy").join("shims");
    std::fs::create_dir_all(&shims).expect("mkdir shims");
    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["doctor"])
        .current_dir(tmp.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PATH", &shims)
        .output()
        .expect("failed to spawn envy doctor");
    assert_eq!(
        out.status.code(),
        Some(0),
        "doctor must exit 0 when clean, stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("all checks passed"),
        "doctor must confirm the clean state"
    );
}

// ---------------------------------------------------------------------------
// 018 — exec injects scoped (Unix: through a real shim on PATH)
// ---------------------------------------------------------------------------

/// Full chain through genuine `PATH` resolution: a `printenv` shim first on
/// `PATH` injects the vault secret into the child only. Pre-existing user
/// shims are never stolen or deleted (only ours is cleaned up).
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
#[cfg(unix)]
fn cli_exec_injects_scoped_through_shim_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let on_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["auto", "on"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn envy auto on");
    assert!(on_out.status.success());

    envy(&["set", "EXEC_TEST_VAR=exec_hello"], tmp.path());

    // Never steal an existing user shim; clean up only what we create.
    let had = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["shim", "list"])
        .current_dir(tmp.path())
        .output()
        .expect("failed to spawn envy shim list");
    let had_printenv = String::from_utf8_lossy(&had.stdout).contains("printenv");
    if !had_printenv {
        let add = Command::new(env!("CARGO_BIN_EXE_envy"))
            .args(["shim", "add", "printenv"])
            .current_dir(tmp.path())
            .output()
            .expect("failed to spawn envy shim add");
        assert!(add.status.success());
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("home dir");
    let shims = std::path::Path::new(&home).join(".envy").join("shims");
    let path = format!(
        "{}:{}",
        shims.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("printenv")
        .arg("EXEC_TEST_VAR")
        .current_dir(tmp.path())
        .env("PATH", &path)
        .stdin(Stdio::null())
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn printenv through shims");
    assert!(
        out.status.success(),
        "shimmed printenv must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).as_ref(),
        "exec_hello\n",
        "child must see the injected secret"
    );

    // Kill-switch: the same shimmed command runs naked (missing var → exit 1).
    let killed = Command::new("printenv")
        .arg("EXEC_TEST_VAR")
        .current_dir(tmp.path())
        .env("PATH", &path)
        .stdin(Stdio::null())
        .env("ENVY_AUTO_INJECT", "0")
        .env_remove("ENVY_ENV")
        .output()
        .expect("failed to spawn printenv with kill-switch");
    assert_eq!(
        killed.status.code(),
        Some(1),
        "kill-switch must run the command without secrets"
    );
    assert!(
        killed.stdout.is_empty(),
        "no secret may leak under the kill-switch"
    );

    if !had_printenv {
        let rm = Command::new(env!("CARGO_BIN_EXE_envy"))
            .args(["shim", "rm", "printenv"])
            .current_dir(tmp.path())
            .output()
            .expect("failed to spawn envy shim rm");
        assert!(rm.status.success());
    }
}

// ---------------------------------------------------------------------------
// 018 — exec injects scoped on Windows (via powershell, no shims needed)
// ---------------------------------------------------------------------------

/// Inject path on Windows: `exec` spawns powershell with the vault secret
/// scoped to the child. Shim-file mechanics are covered by the hermetic
/// reshim test plus unit tests.
#[test]
#[ignore = "requires a live OS keyring daemon (Secret Service / Keychain)"]
#[cfg(windows)]
fn cli_exec_injects_scoped_on_windows() {
    let tmp = tempfile::tempdir().expect("tempdir");
    setup_project(tmp.path());

    let on_out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args(["auto", "on"])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn envy auto on");
    assert!(on_out.status.success());

    envy(&["set", "EXEC_TEST_VAR=exec_hello"], tmp.path());

    let out = Command::new(env!("CARGO_BIN_EXE_envy"))
        .args([
            "exec",
            "--",
            "powershell",
            "-NoProfile",
            "-Command",
            "Write-Output $env:EXEC_TEST_VAR",
        ])
        .current_dir(tmp.path())
        .stdin(Stdio::null())
        .env_remove("ENVY_ENV")
        .env_remove("ENVY_AUTO_INJECT")
        .output()
        .expect("failed to spawn envy exec");
    assert!(
        out.status.success(),
        "envy exec must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "exec_hello",
        "child must see the injected secret"
    );
}
