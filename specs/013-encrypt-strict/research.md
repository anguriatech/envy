# Research: Strict `envy encrypt` (No Silent Key Rotation)

**Feature**: 013-encrypt-strict
**Date**: 2026-06-10
**Status**: All clarifications resolved (3 clarifications provided by user in the planning input)

## Summary

Tighten the contract of `envy encrypt` so it can no longer silently rotate the passphrase of an existing envelope. The new behaviour: first-time seal (new env), re-seal with matching passphrase (update), or fail clearly (mismatch → exit 2 + hint pointing to `envy rotate`). The interactive-only `confirm_key_rotation` prompt is removed entirely. The `envy rotate` command (spec 012) is the dedicated, safe path for key rotation.

All technical decisions are derived from the user-supplied clarifications and the existing code patterns in the project. No `NEEDS CLARIFICATION` markers remain.

## Decisions

### Decision 1: Error message formatting (clarification #1)

**Decision**: The `CliError::PassphraseInput` variant is constructed with a `String` that contains a literal `\n` between the error sentence and the hint. The `error: ` prefix is NOT added inside the `CliError` payload — it is added by `format_cli_error` at `src/cli/error.rs:86-88`. The exact construction is:

```rust
return Err(CliError::PassphraseInput(format!(
    "passphrase does not match the existing envelope.\nhint: use `envy rotate -e ENV` to change the envelope's passphrase."
)));
```

**Rationale**:
- `format_cli_error` at `src/cli/error.rs:86-88` is `format!("error: {e}")` — it prepends `"error: "` to whatever the `Display` impl produces. If we include `"error: "` inside the `CliError` payload, the user would see `"error: error: passphrase does not match..."`.
- The `Display` impl for `CliError::PassphraseInput(String)` (via `#[error("passphrase input failed: {0}")]` at `src/cli/error.rs:48-49`) wraps the payload in `"passphrase input failed: <payload>"`. So the final user-visible output is:
  ```
  error: passphrase input failed: passphrase does not match the existing envelope.
  hint: use `envy rotate -e ENV` to change the envelope's passphrase.
  ```
- The `format_cli_error` formatter and the `Display` impl are both correct as-is. We do NOT modify them.

**Alternatives considered**:
- *Embed "error: " inside the CliError*: rejected — would produce `"error: error: ..."`.
- *Add a new CliError variant for this specific case*: rejected by the user constraint "no new error variants".
- *Override the Display impl just for this case*: rejected — over-engineering, and the `PassphraseInput` variant is semantically correct (it's a passphrase-input failure, mapped to exit 2).

**Final user-visible output** (the spec's SC-003 acceptance test):
```
error: passphrase input failed: passphrase does not match the existing envelope.
hint: use `envy rotate -e ENV` to change the envelope's passphrase.
```

### Decision 2: Empty-vault guard position (clarification #2)

**Decision**: The empty-vault guard runs FIRST in the per-env loop, BEFORE the verify step. Sequence:
1. Empty-vault guard (skip with warning if 0 secrets)
2. If envelope exists, verify passphrase
3. If envelope does not exist, proceed to create
4. Seal

**Rationale**:
- This is the spec's User Story 5 consistency rule: empty-vault applies in BOTH new-envelope and update-envelope cases.
- The existing code at `src/cli/commands.rs:727-736` already has the empty-vault guard at the top of the loop. The new verify block goes AFTER it, not before.
- The order matters: if a user has 0 secrets locally and runs `envy encrypt` against an existing envelope with the correct passphrase, they get the "nothing to seal" warning instead of accidentally overwriting a real envelope with an empty one. This is the safety property the spec adds.

**Alternatives considered**:
- *Verify first, then empty-vault check*: rejected — the user would be prompted for a passphrase (interactive) or consume an env var (headless) before being told "actually, nothing to seal". The new order skips both the prompt and the env-var consumption, which is faster and more discoverable.
- *Combine empty-vault + verify into one helper*: rejected — adds a new core helper, which violates the "no new core helpers" guidance in the user's constraints.

### Decision 3: `confirm_key_rotation` removal (clarification #3)

**Decision**: The `confirm_key_rotation` function at `src/cli/commands.rs:551-570` (function definition + its 6-line doc comment) is DELETED entirely. The single call site at `src/cli/commands.rs:758` is also replaced by the new verify-or-fail block. After deletion, `grep -r confirm_key_rotation src/` returns zero matches. This is hard SC-006.

**Rationale**:
- The spec explicitly states the function must be removed, not just unused.
- The function is only called from `cmd_encrypt` (the spec verified this with a grep).
- Leaving the function as dead code would trigger clippy's `dead_code` lint and would bloat the binary.
- The `Dialoguer::Confirm` import in `commands.rs` should also be removed if it's no longer used elsewhere — verified: the only usage is in `confirm_key_rotation` (other interactive prompts use `dialoguer::Password`, not `dialoguer::Confirm`).

**Alternatives considered**:
- *Leave the function as `#[allow(dead_code)]`*: rejected by the spec.
- *Move the function to a `#[cfg(test)]` module for documentation*: rejected — the function is not used in any test either.

**Implementation step**: a single multi-line `edit` call to remove lines 551-570 and the call site at 758, plus a removal of the `dialoguer::Confirm` import (if applicable). The exact diff will be applied during implementation.

### Decision 4: No new core helpers, no new error variants

**Decision**: The verify-or-fail block in `cmd_encrypt` calls the existing `crate::core::check_envelope_passphrase(...)` (at `src/core/sync.rs:348-354`) directly. No new helper is added. The mismatch result is mapped to the existing `CliError::PassphraseInput(String)` variant.

**Rationale**:
- `check_envelope_passphrase` already does exactly what we need: it returns `true` if the passphrase unseals the envelope, `false` otherwise (it returns `false` for both wrong-passphrase and tampered-ciphertext, which is the right behaviour for a verify step).
- The user's constraint explicitly says: "Do NOT add a new core helper; this is just a one-line function call from cmd_encrypt."
- The existing `CliError::PassphraseInput(String)` variant already maps to exit 2 (via `cli_exit_code` at `src/cli/error.rs:135`).

**Alternatives considered**:
- *Add a new `core::verify_envelope` helper*: rejected by the user's constraint.
- *Add a new `CliError::PassphraseMismatch` variant*: rejected by the user's constraint.

### Decision 5: The Diceware flow is preserved

**Decision**: The Diceware suggestion + banner flow at `src/cli/commands.rs:766-770` is preserved unchanged. The Diceware suggestion is only generated for `is_new_env` (not for update case), which is consistent with the existing logic. The `print_diceware_banner` call is at the right place: AFTER the verify step, so the banner is only shown if the passphrase was accepted (whether new or matching).

**Rationale**:
- The spec's "What MUST stay" section explicitly lists "The Diceware passphrase suggestion for new envelopes (spec 009 FR in `envy encrypt` interactive mode) is preserved."
- The Diceware suggestion is only generated for `is_new_env` (the existing code at lines 717-723 already filters this), so for the update case the banner is never shown. This is correct: in the update case, the user is verifying an existing passphrase, not creating a new one.

**Implementation note**: no code change required for Diceware. It already works correctly with the new verify block.

### Decision 6: Test changes

**Decision**:
- **Update existing tests** that reference "ROTATE" or "key-rotation warning" — there is exactly one such test, at `src/cli/commands.rs:2084-2093` (a test of `check_envelope_passphrase` itself, not of `cmd_encrypt`). The assertion text is updated to remove the "key-rotation warning path" reference, but the test logic is unchanged.
- **Add 9 new tests** as listed in the user's "Tests strategy" section. The tests cover US1-US5 acceptance scenarios plus the SHA-256 invariant (SC-003).
- **Update E2E scenario**: add Scenario 11 (or append to Scenario 5 / 10) that covers the new strict behaviour. No existing E2E scenario tests the old "silent rotation" path, so this is purely additive.

**Rationale**:
- The user's constraint explicitly requires the test changes. The existing test at line 2084 is the only test that mentions "key-rotation warning" in its assertion text; the update is a one-line change.
- The new tests use the existing `TEST_MASTER_KEY` and `ENV_LOCK` pattern in the `mod tests` block of `src/cli/commands.rs` (around line 1500+).
- The E2E test uses the same headless pattern as Scenario 10 (env vars, `< /dev/null`, sha256sum).

**Implementation note**: the tests are added after the existing `encrypt_skips_empty_env_with_warning` test (around line 2289 in the current file).

### Decision 7: Documentation changes

**Decision**:
- `README.md` — update the `envy encrypt` row in the command table (line 309) to mention that the passphrase must match an existing envelope. Add a short paragraph in the "Multi-Environment with Separate Passphrases" section (line 362) explaining the strict behaviour.
- `docs/developer-guide.md` — add a paragraph to the GitOps section (around line 76) noting the strict behaviour. The existing §7.5 on `envy rotate` is unchanged; the spec is purely additive.

**Rationale**:
- The README is the primary user-facing documentation. The encrypt command's behaviour is changing in a backwards-incompatible way (per the spec, the project's pre-1.0 status makes this acceptable).
- The developer guide's GitOps section is the right place to explain the encrypt/rotate separation at a technical level.

**Alternatives considered**:
- *Add a CHANGELOG.md*: the project does not have one (this was flagged in the 006 review). The commit message and the README update are sufficient for now.
- *Add a new doc file*: not needed — the existing files have the right structure.

### Decision 8: Version bump

**Decision**: `Cargo.toml` bumps from `0.3.0` to `0.3.1` (patch). The cargo-dist release workflow picks up the new tag automatically.

**Rationale**:
- The user's "Versioning" section explicitly specifies this.
- The behaviour tightening on an existing command is a patch bump per the project's pre-1.0 versioning convention.

## Architectural Constraints Honoured

| Constraint | How it is honoured |
|------------|-------------------|
| 4-layer separation: cli → core → {crypto, db} | Verify logic lives in `cmd_encrypt` (cli); the verify call uses `crate::core::check_envelope_passphrase` (core); no new core helper. |
| No new crate dependencies | No changes to `Cargo.toml` dependencies. |
| No new error variants | Reuse `CliError::PassphraseInput` (existing, maps to exit 2). |
| No new CLI flags | `Commands::Encrypt` is unchanged. |
| All 19 existing command IDs preserved | The `envy rotate` subcommand (spec 012) and all 18 pre-existing commands are unchanged. |
| No modification of `seal_artifact` / `seal_env` / `check_envelope_passphrase` | All three helpers in `src/core/sync.rs` are reused as-is. |
| `confirm_key_rotation` deleted | The function and its doc comment are removed; the call site is replaced by the new verify-or-fail block. |

## Out-of-Scope Confirmations

The user explicitly listed the following as out-of-scope for this plan. They are not designed here:

- Removing or modifying the `envy rotate` interactive prompt (preserved unchanged per spec 012).
- Adding a `--force-rotate` or `--allow-rotation` flag to `envy encrypt`.
- Changing `envy decrypt` behaviour.
- Modifying the VS Code extension.
- Spec 006 acceptance scenarios about "key rotation warning" in encrypt (the spec is unchanged; only the implementation in encrypt changed).

## References

- Spec: `specs/013-encrypt-strict/spec.md`
- Constitution: `.specify/memory/constitution.md`
- `confirm_key_rotation` (to be deleted): `src/cli/commands.rs:551-570`
- `confirm_key_rotation` call site (to be replaced): `src/cli/commands.rs:748-763`
- `check_envelope_passphrase` (reused): `src/core/sync.rs:348-354`
- `resolve_passphrase_for_env` (reused): `src/cli/commands.rs:412`
- `format_cli_error` (adds `error: ` prefix): `src/cli/error.rs:86-88`
- `CliError::PassphraseInput` (reused, maps to exit 2): `src/cli/error.rs:48-49` + `src/cli/error.rs:135`
