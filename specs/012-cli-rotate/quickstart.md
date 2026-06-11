# Quickstart: Implementing Envelope Passphrase Rotation

**Feature**: 012-cli-rotate
**Date**: 2026-06-10

This is a step-by-step implementation guide for the developer picking up the task. Read this before opening the codebase.

## Pre-flight

1. Make sure you are on the `012-cli-rotate` branch (created by the `before_specify` hook):
   ```bash
   git branch --show-current
   # Expected: 012-cli-rotate
   ```
2. Read the spec: `specs/012-cli-rotate/spec.md`.
3. Read the research: `specs/012-cli-rotate/research.md`.
4. Read the data model: `specs/012-cli-rotate/data-model.md`.
5. Read the contracts: `specs/012-cli-rotate/contracts/cli-rotate.md` and `contracts/core-rotate-env.md`.

## Files to modify (in order)

1. `src/core/sync.rs` — add `rotate_env` (after `seal_env`, before `check_envelope_passphrase`).
2. `src/cli/commands.rs` — add `cmd_rotate` (after `cmd_decrypt`).
3. `src/cli/mod.rs` — add `Rotate` variant to `Commands` enum + dispatch arm in `run()`.
4. `src/cli/commands.rs` (tests module) — add 7 unit tests for `cmd_rotate`.
5. `tests/e2e_devops_scenarios.sh` — add scenario 10: rotation.
6. `README.md` — add `envy rotate` to the command table and to the "Multi-Environment with Separate Passphrases" section.
7. `docs/developer-guide.md` — mention `rotate` in the GitOps section and the cryptographic flow.
8. `Cargo.toml` — bump version to `0.3.0`.

## Implementation order

### Step 1: `core::sync::rotate_env`

Open `src/core/sync.rs`. Locate the `seal_env` function (around line 225). Add a new `rotate_env` function after it (around line 262, just before `check_envelope_passphrase`).

The function signature is:

```rust
pub fn rotate_env(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    artifact: &mut SyncArtifact,
    env_name: &str,
    current_passphrase: &str,
    new_passphrase: &str,
) -> Result<(), SyncError>
```

The body follows the algorithm in `contracts/core-rotate-env.md`. The key calls are:

- `check_envelope_passphrase(current_passphrase, env_name, existing_envelope)` to verify the current passphrase.
- `seal_env(vault, master_key, project_id, env_name, new_passphrase)` to produce the new envelope (this also writes the sync marker for free).
- Insert the new envelope into `artifact.environments[env_name]`.

Add doc-comments referencing the spec's FR-005, FR-016, FR-017, FR-018.

### Step 2: `cli::commands::cmd_rotate`

Open `src/cli/commands.rs`. Locate `cmd_decrypt` (around line 814). Add a new `cmd_rotate` function after it.

The function signature is:

```rust
pub(super) fn cmd_rotate(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    artifact_path: &std::path::Path,
    env_filter: Option<&str>,
) -> Result<(), CliError>
```

The body follows the contract in `contracts/cli-rotate.md`. Key points:

- **Headless detection**: implement a small helper `is_rotate_headless_mode(env_name) -> bool` that returns `true` only when BOTH `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` are set and non-empty.
- **Env selection**: if `env_filter` is `Some`, use that single env. Else, present a `MultiSelect` of all envs in the artifact (read it via `crate::core::read_artifact(artifact_path)`).
- **For each selected env**:
  1. **Empty-env guard**: mirror the `cmd_encrypt` pattern at lines 727-736.
  2. **Resolve current passphrase**: call `resolve_passphrase_for_env(env_name, false, None)`. If it returns `None` (no env var and no TTY), return `CliError::PassphraseInput("envy rotate requires ...")`.
  3. **Resolve new passphrase** (interactive only): `dialoguer::Password::with_theme(&theme).with_prompt("New passphrase for '<env>'").with_confirmation("Confirm new passphrase", "Passphrases do not match.").interact()`. Wrap the result in `Zeroizing::new(...)` immediately. If the result is `Err`, return `CliError::PassphraseInput(...)`.
  4. **Resolve new passphrase** (headless): read `ENVY_PASSPHRASE_<ENV>_NEW` directly. Wrap in `Zeroizing::new(...)`. Reject whitespace-only.
  5. **Verify new != current**: if the new passphrase equals the current passphrase, return `CliError::PassphraseInput("new passphrase must differ from the current passphrase")`.
  6. **Call `crate::core::rotate_env(...)`**: this verifies the current passphrase and re-seals. Map the `SyncError` variants to `CliError` as described in the contract.
  7. **Print success line**: `  ✓  '<env>' rotated. Passphrase changed.` and a second line with the forward-only note.
- **Atomic write**: after the loop, call `crate::core::write_artifact_atomic(&artifact, artifact_path)`.

**Memory hygiene note**: the `Zeroizing<String>` bindings for `current` and `new` must be the only references to the passphrase data. Do not `.clone()` them, do not store them in `&str` that outlives the function scope, do not pass them to `println!` or `eprintln!` (the success message does not include the passphrase value).

### Step 3: `cli::mod::Commands::Rotate`

Open `src/cli/mod.rs`. Add a new variant to the `Commands` enum (just before `Completions`):

```rust
/// Re-seal an existing envelope with a new passphrase.
///
/// The current passphrase is verified before the new one is accepted.
/// Prompts for the current, new, and confirmation passphrases interactively,
/// or reads `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` in CI.
Rotate {
    /// Target environment to rotate (default: MultiSelect from envy.enc).
    #[arg(short = 'e', long = "env", value_name = "ENV")]
    env: Option<String>,
},
```

Then add a dispatch arm in `run()` (just before `Commands::Completions`):

```rust
Commands::Rotate { env } => {
    let artifact_path = artifact_path(&manifest_path);
    match commands::cmd_rotate(
        &vault,
        &master_key,
        &project_id,
        &artifact_path,
        env.as_deref(),
    ) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", format_cli_error(&e));
            cli_exit_code(&e)
        }
    }
}
```

### Step 4: Unit tests

In the tests module of `src/cli/commands.rs` (around line 1500+), add the 7 tests listed in `research.md` Decision 10. Use the existing `TEST_MASTER_KEY` and `ENV_LOCK` pattern. The tests do not need an OS keyring.

### Step 5: E2E scenario

In `tests/e2e_devops_scenarios.sh`, add a new section "Scenario 10 — Envelope Passphrase Rotation" that:

1. Initialises a fresh project.
2. Sets a secret.
3. Encrypts with a passphrase `A` (via `ENVY_PASSPHRASE_DEVELOPMENT=A`).
4. Rotates with `envy rotate -e development` using `ENVY_PASSPHRASE_DEVELOPMENT=A` + `ENVY_PASSPHRASE_DEVELOPMENT_NEW=B` (headless).
5. Asserts exit code 0 and that the success line was printed.
6. Re-encrypts with `B` (to verify the new passphrase works).
7. Asserts exit code 0.
8. Tests the wrong-current-passphrase path: try to rotate with `A` against the new `B`-sealed envelope, assert exit code 2 and that `envy.enc` is byte-identical to the post-rotation state.

### Step 6: Documentation

- `README.md`: add `envy rotate [-e ENV]` to the command table.
- `README.md`: in the "Multi-Environment with Separate Passphrases" section, add a short paragraph explaining rotation.
- `docs/developer-guide.md`: in the GitOps section, add a paragraph about `envy rotate` as the safe path for key rotation. In the cryptographic flow section, mention that rotation re-seals via `seal_env` (no new crypto primitives needed).

### Step 7: Version bump

In `Cargo.toml`, bump `version` from `0.2.7` to `0.3.0`. The cargo-dist release workflow picks up the new tag automatically on the next release.

## Verification

Run these commands locally before opening the PR:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
ENVY_BIN=$(pwd)/target/debug/envy bash tests/e2e_devops_scenarios.sh
```

The CI quality gate runs all of the above. Make sure they pass locally first to avoid a CI round-trip.

## Layer check (Constitution Principle IV)

Verify before opening the PR:

- `src/cli/commands.rs::cmd_rotate` MUST NOT import from `crate::crypto` or `crate::db` directly. It calls `crate::core::rotate_env` and `crate::core::write_artifact_atomic` (the only permitted exceptions are `crate::db::Vault::open` in `run()` and the helpers already used by `cmd_decrypt`).
- `src/core/sync.rs::rotate_env` MUST NOT import from `crate::cli`. It calls `crate::core::get_env_secrets` and `crate::crypto::seal_envelope` only.

## Security gate (Constitution Principle I)

Verify before opening the PR:

- The current and new passphrases are wrapped in `zeroize::Zeroizing<String>`.
- They are dropped before any early `return` statement in `cmd_rotate`.
- The success message does NOT include either passphrase.
- The error messages do NOT include either passphrase.
- No `println!` / `eprintln!` / `dbg!` / `log::*` call takes the passphrase as an argument.

## No-panic audit (Constitution Principle III)

Verify before opening the PR:

- The only `.unwrap()` calls in `cmd_rotate` and `rotate_env` are the ones already present in the helpers we reuse (`is_rotate_headless_mode`, `resolve_passphrase_for_env`, `seal_env`, `check_envelope_passphrase`). No new `.unwrap()` is introduced.
- If any new `.expect()` is introduced, it has an inline comment justifying why the surrounding code makes it statically impossible to panic.
