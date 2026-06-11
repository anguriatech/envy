# Research: Envelope Passphrase Rotation

**Feature**: 012-cli-rotate
**Date**: 2026-06-10
**Status**: All clarifications resolved (3 clarifications provided by user in the planning input)

## Summary

The new `envy rotate` command re-seals a single envelope (or several) in `envy.enc` with a new passphrase, after verifying the current passphrase matches the existing envelope. This fills the safety gap left by `envy encrypt`'s silent key-rotation behaviour in headless mode (used by the VS Code extension, where a typo silently changes the envelope's passphrase and locks the team out).

All technical decisions are derived from the user-supplied clarifications and the existing code patterns already in the project. No `NEEDS CLARIFICATION` markers remain.

## Decisions

### Decision 1: Headless vs TTY precedence (clarification #1)

**Decision**: When BOTH `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` are set AND a TTY is present, the command MUST prefer headless mode (use env vars, skip the prompts).

**Rationale**: This matches the existing `envy encrypt` behaviour at `src/cli/commands.rs:680` (the `is_headless_mode(...)` call). Consistency with the existing command is more valuable than "asking the user again" when env vars are set; operators explicitly provided the env vars for a reason.

**Alternatives considered**:
- *TTY wins*: Reject the env vars if a TTY is available. Rejected — would surprise CI users who also happen to run from a terminal.
- *Error on conflict*: Fail with a clear error. Rejected — adds friction without value; users have one legitimate preference (env vars), and the precedence is unambiguous.
- *Prompt as a fallback*: Use env vars first, prompt only if the env vars yield an invalid result. Rejected — silently failing over to a TTY prompt in a CI job is the exact failure mode this spec is trying to avoid.

**Implementation pattern**: The `is_headless_mode` helper at `src/cli/commands.rs:513` already returns `true` when any per-env passphrase env var is non-empty. We extend the helper semantics for `rotate` to also require the corresponding `_NEW` env var. The cleanest way is a separate `is_rotate_headless_mode(&[String]) -> bool` helper in `commands.rs` that checks BOTH `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` for each env. Keeping the helpers separate avoids overloading `is_headless_mode` with semantics that don't apply to `encrypt`.

### Decision 2: Success message format (clarification #2)

**Decision**: The success line MUST include the rotated environment name. The exact format is:

```
  ✓  'production' rotated. Passphrase changed.
     Previous passphrase can no longer decrypt this artifact.
```

The first line is the headline; the second is the "forward-only" note. The check-mark uses the same `dialoguer::console::style("✓").green()` style as `cmd_decrypt` (line 901).

**Rationale**:
- The user explicitly specified the format.
- Including the env name is critical for multi-environment rotations, where the user needs to know which envs succeeded.
- The "forward-only" note is a spec-mandated transparency requirement (FR-015) — operators must understand that the old passphrase is dead and artifacts sealed with it cannot be recovered.

**Alternatives considered**:
- *Single-line success*: `"rotated 'production'"`. Rejected — does not satisfy FR-015.
- *Two-line but with the "forward-only" warning in a separate Y/N confirm prompt*: Rejected — adds friction, the warning is informational, not a decision point.
- *Three-line (env, action, warning)*: Rejected — over-verbose for a success path.

**Output sink**: `stdout` for the success line, so it composes correctly in CI pipelines (the rest of envy's success output also goes to `stdout`).

### Decision 3: Memory hygiene for passphrases (clarification #3)

**Decision**: The current and new passphrases MUST be wrapped in `zeroize::Zeroizing<String>` inside `cmd_rotate`. They MUST be dropped before any early return. The existing `resolve_passphrase_for_env` already returns `Zeroizing<String>`; `cmd_rotate` MUST call it for both passphrases.

**Rationale**:
- Constitution Principle I (Security by Default) requires in-memory secret representations to be zeroed/dropped as early as possible.
- The existing `resolve_passphrase_for_env` at `src/cli/commands.rs:412` already returns `Result<Option<Zeroizing<String>>, CliError>`. Reusing it for the current passphrase is the obvious choice. For the new passphrase, we use a fresh `dialoguer::Password::with_confirmation` call and wrap the result in `Zeroizing::new(...)` before storing it in a local variable.
- Storing the new passphrase in a `Zeroizing<String>` ensures that when the binding goes out of scope (or the function returns), the underlying heap memory is overwritten with zeros — even on panic, because `Zeroizing` implements `Drop` with explicit `ptr::write_bytes(0, ...)`.

**Alternatives considered**:
- *Read the new passphrase via `std::io::stdin().read_to_string(...)` and wrap*: Rejected — `dialoguer::Password` already provides hidden-input prompts (no echo to terminal) and double-entry confirmation via `Password::with_confirmation`. Reading raw from stdin would lose the hidden-input feature.
- *Pass `&str` references all the way down to `core::sync::rotate_env`*: Rejected — the function signature should accept `&str` (the inner `seal_envelope` takes `&str` for the passphrase), but the call site in `cmd_rotate` keeps ownership of the `Zeroizing<String>` binding. The `&str` view is constructed on demand via `.as_ref()` and never outlives the `Zeroizing` binding in the same scope.

**Implementation pattern**:
```rust
// In cmd_rotate
let current: Zeroizing<String> = match resolve_passphrase_for_env(env_name, false, None)? {
    Some(p) => p,
    None => return Err(CliError::PassphraseInput(
        "envy rotate requires either a TTY or ENVY_PASSPHRASE_<ENV>".into(),
    )),
};
// current is dropped at end of scope or on early return (Zeroizing::drop)
```

### Decision 4: `core::sync::rotate_env` signature

**Decision**: The new helper in `src/core/sync.rs` is:

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

The function:
1. Validates that `env_name` is in `artifact.environments` (else `SyncError::Artifact(ArtifactError::MalformedArtifact("env not in artifact")` mapped to `CliError::EnvNotFound` at the call site).
2. Calls `check_envelope_passphrase(current_passphrase, env_name, &existing_envelope)` to verify the current passphrase. If it returns `false`, returns `Err(SyncError::Artifact(ArtifactError::DecryptionFailed))` (a new dedicated variant — see "no new error variants" note below).
3. Calls `seal_env(vault, master_key, project_id, env_name, new_passphrase)` to re-seal (this re-reads the vault, generates a fresh nonce, and writes a fresh `sync_marker`).
4. Inserts the new envelope into `artifact.environments` (replacing the old one).

**Rationale**:
- The 4-layer rule (Constitution Principle IV) requires the CLI not to call `crypto::seal_envelope` directly. The new `rotate_env` is the core-level helper that wraps the verify-then-seal pattern.
- Reusing `seal_env` (rather than calling `seal_envelope` directly) gives us the `sync_marker` update for free — `envy status` will report the new envelope as `InSync` immediately after a successful rotation, which is the correct user expectation.

**Alternatives considered**:
- *Inline implementation in `cmd_rotate`*: Rejected — violates the 4-layer rule.
- *New helper `rotate_in_place` that takes the plaintext secrets and re-seals*: Rejected — would re-implement the `seal_env` logic, duplicating the sync_marker write. Reusing `seal_env` is the right factoring.
- *Verify the current passphrase at the CLI layer*: Rejected — the `check_envelope_passphrase` function is in `core::sync` and that's where the verification logic should live. The CLI layer passes the passphrases in and gets a typed `Result` back.

**No-new-error-variants note**: The user stated "No new error variants expected". Mapping the "wrong current passphrase" case to `SyncError::Artifact(ArtifactError::DecryptionFailed)` reuses an existing variant. At the CLI boundary, we map it to `CliError::PassphraseInput("current passphrase does not match the existing envelope")` — reusing the existing `PassphraseInput` variant that already maps to exit code 2.

### Decision 5: Atomic write reuse

**Decision**: `cmd_rotate` reuses the existing `write_artifact_atomic` helper at `src/core/sync.rs:201`. No new atomic-write logic is introduced.

**Rationale**: The spec requires atomic write (FR-016). The existing helper already provides it (writes to `envy.enc.tmp` then renames). Reusing it satisfies the spec requirement and keeps the codebase consistent.

**Alternatives considered**:
- *Write directly via `std::fs::write`*: Rejected — would violate FR-016 and risk a partial-write scenario.

### Decision 6: Empty-env guard pattern

**Decision**: `cmd_rotate` mirrors the exact pattern at `src/cli/commands.rs:727-736`:

```rust
let secret_keys = crate::core::list_secret_keys(vault, project_id, env_name).unwrap_or_default();
if secret_keys.is_empty() {
    eprintln!("  {}  environment '{}' has 0 secrets, skipping",
        dialoguer::console::style("\u{26a0}").yellow(),
        env_name);
    continue; // (in multi-env) or return Ok(()) (in single-env)
}
```

**Rationale**:
- The spec requires the empty-env guard (FR-010).
- The pattern is identical to `cmd_encrypt` — consistency is more important than variation.

### Decision 7: Multi-environment rotation

**Decision**: When no `-e` flag is provided, `cmd_rotate` mirrors `cmd_encrypt`'s `MultiSelect` pattern (lines 688-703). The user can select one or more environments. For each selected env, the current/new/confirm prompts are repeated in turn.

**Rationale**: This is the same UX as `cmd_encrypt`, and the spec explicitly calls for it (User Story 4).

### Decision 8: No `confirm_key_rotation` reuse

**Decision**: The existing `confirm_key_rotation` helper at `src/cli/commands.rs:557` is NOT reused for `cmd_rotate`. The new command uses a different prompt order (current → new → confirm).

**Rationale**: The `confirm_key_rotation` helper is a Y/N decision prompt ("Rotate the key for 'ENV'?"). `cmd_rotate` does not have a Y/N decision — it has a structured current/new/confirm passphrase flow. Reusing `confirm_key_rotation` would be the wrong shape. The spec explicitly notes that the existing key-rotation warning in `cmd_encrypt` becomes redundant after this spec ships, but is preserved for backward compatibility.

### Decision 9: Version bump

**Decision**: `Cargo.toml` version bumps from `0.2.7` (current `master`, set by the recent `--stdin` PR) → `0.3.0`. The user explicitly mentioned `0.2.6 → 0.3.0`, but the actual current value is `0.2.7`. The plan uses `0.3.0` as the user specified, which is consistent with the semantic-versioning policy: adding a new subcommand is a minor (not patch) bump.

**Rationale**: A new subcommand is additive and backward-compatible, so MINOR is correct per semver. PATCH would not reflect the new functionality.

**Note on cargo-dist release workflow**: The `release.yml` workflow picks up the new tag automatically. The `version` field in `Cargo.toml` is the source of truth for the next release tag. No workflow changes required.

### Decision 10: Test strategy

**Decision**:
- **Unit tests** in `src/cli/commands.rs` test module (around line 1500+), using the existing `TEST_MASTER_KEY` and `ENV_LOCK` pattern. Tests do NOT need an OS keyring.
- **E2E test** in `tests/e2e_devops_scenarios.sh` covers the happy path + the wrong-current-passphrase guard. The E2E suite runs in CI inside `dbus-run-session` for headless keyring access.

**Rationale**: The existing test pattern uses `tempfile::tempdir()` + `Vault::open` with a fixed master key. This is sufficient to test the rotation logic without depending on the OS keyring. The E2E test covers the "real" CLI invocation end-to-end.

**Test cases to add** (all unit tests unless noted):
1. `rotate_happy_path_interactive_prompts` — seals with `passA`, rotates to `passB`, verifies decrypt with `passB` works and decrypt with `passA` fails.
2. `rotate_wrong_current_passphrase_leaves_artifact_unchanged` — verifies SHA-256 of `envy.enc` before and after a failed rotation are identical.
3. `rotate_new_equals_current_is_rejected` — verifies the error and that the artifact is unchanged.
4. `rotate_confirmation_mismatch_is_rejected` — verifies the error and that the artifact is unchanged.
5. `rotate_headless_with_env_vars_succeeds` — uses `ENVY_PASSPHRASE_<ENV>` + `ENVY_PASSPHRASE_<ENV>_NEW` to drive the rotation in headless mode.
6. `rotate_multi_environment_rotates_each` — selects two envs, verifies both envelopes are re-sealed.
7. `rotate_empty_env_skips_with_warning` — env has 0 secrets in vault, verifies a warning and unchanged artifact.
8. `rotate_integration_rotate_then_decrypt` — E2E: rotate, then decrypt with new (succeeds), decrypt with old (fails).
9. E2E scenario 10 in `tests/e2e_devops_scenarios.sh` — "Pre-Rotate Verification & Rotation" — covers the happy path and the wrong-current-passphrase guard.

## Architectural Constraints Honoured

| Constraint | How it is honoured |
|------------|-------------------|
| 4-layer separation: cli → core → {crypto, db} | `cmd_rotate` lives in `src/cli/commands.rs`; the verify-then-seal logic is in `src/core/sync.rs` as `rotate_env`; `cmd_rotate` does not call `crypto::seal_envelope` directly. |
| No new crate dependencies | Only existing crates used: `dialoguer` (Password + MultiSelect), `serde_json`, `zeroize`, `thiserror`, `clap`. |
| Existing helpers reused | `resolve_passphrase_for_env` (current passphrase), `check_envelope_passphrase` (verify current), `write_artifact_atomic` (atomic write), `seal_env` (re-seal with sync marker). |
| Empty-env guard | Mirrors `cmd_encrypt` pattern at `src/cli/commands.rs:727-736`. |
| No `ENVY_PASSPHRASE_NEW` global | Only the per-env pair `ENVY_PASSPHRASE_<ENV>` + `ENVY_PASSPHRASE_<ENV>_NEW` is supported. |
| All 18 existing command IDs preserved | The new `Rotate` variant is purely additive. No existing variant is renamed, hidden, or modified. |
| No new error variants expected | All errors map to existing `CliError` variants: `PassphraseInput` (exit 2), `EnvNotFound` (exit 3), `FileNotFound` (exit 1), `VaultOpen` (exit 4). |

## Out-of-Scope Confirmations

The user explicitly listed the following as out-of-scope for this plan. They are not designed here:

- Removing or modifying the existing `confirm_key_rotation` prompt in `cmd_encrypt` — preserved for backward compat.
- Rotation audit log — no historical record is added.
- Revocation semantics — rotation is forward-only, the spec says so.
- Distributing the new passphrase — team's responsibility.
- Changes to `envy-vscode` — a follow-up spec will add a "Envy: Rotate Passphrase" command to the extension.

## References

- Spec: `specs/012-cli-rotate/spec.md`
- Constitution: `.specify/memory/constitution.md`
- Existing `seal_env` (to reuse): `src/core/sync.rs:225`
- Existing `check_envelope_passphrase` (to reuse): `src/core/sync.rs:275`
- Existing `write_artifact_atomic` (to reuse): `src/core/sync.rs:201`
- Existing `resolve_passphrase_for_env` (to reuse): `src/cli/commands.rs:412`
- Existing empty-env guard pattern: `src/cli/commands.rs:727-736`
- Existing `is_headless_mode` (model for the new helper): `src/cli/commands.rs:513`
- Existing CliError variants: `src/cli/error.rs:22`
- Existing exit-code mapping: `src/cli/error.rs:127`
