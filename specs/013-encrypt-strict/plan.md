# Implementation Plan: Strict `envy encrypt` (No Silent Key Rotation)

**Branch**: `013-encrypt-strict` | **Date**: 2026-06-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/013-encrypt-strict/spec.md`

## Summary

Tighten the contract of `envy encrypt` so it can no longer silently rotate the passphrase of an existing envelope. The new behaviour: first-time seal (new env), re-seal with matching passphrase (update), or fail clearly (mismatch → exit 2 + hint pointing to `envy rotate`). The interactive-only `confirm_key_rotation` prompt is removed entirely. The `envy rotate` command (spec 012) is the dedicated, safe path for key rotation.

The implementation is purely behavioural tightening on an existing command: no new flags, no new error variants, no new crate dependencies, no new core helpers. The change is localised to `src/cli/commands.rs::cmd_encrypt` plus the deletion of the unused `confirm_key_rotation` function.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.85)
**Primary Dependencies**: `clap` (derive), `dialoguer` (Password + MultiSelect), `serde_json`, `zeroize`, `thiserror`, `toml` — all already in `Cargo.toml`. No new crate added.
**Storage**: SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (existing vault, read-only for this feature). `envy.enc` artifact on disk (JSON, atomic write via existing helper).
**Testing**: `cargo test` for unit and integration tests; `tests/e2e_devops_scenarios.sh` for E2E. Existing `TEST_MASTER_KEY` + `ENV_LOCK` pattern in `src/cli/commands.rs:1544` is reused. `sha2 = "0.10"` was added as a dev-dependency in spec 012.
**Target Platform**: Linux + macOS + Windows (already supported by envy; this spec inherits the same platform support).
**Project Type**: CLI tool (single binary, statically linked).
**Performance Goals**: < 5 seconds for a single-env seal in interactive mode (dominated by Argon2id KDF cost). No new performance work needed.
**Constraints**: 
- No new error variants
- No new CLI flags
- No new crate dependencies
- No new core helpers (reuse `check_envelope_passphrase`)
- `confirm_key_rotation` function must be DELETED (not just unused)
- All 19 existing command IDs preserved (`init`, `set`, `get`, `list`, `ls`, `rm`, `remove`, `unset`, `run`, `migrate`, `encrypt`, `enc`, `decrypt`, `dec`, `export`, `diff`, `df`, `status`, `st`, `completions`, `rotate`)
- The `envy rotate` subcommand (spec 012) is unchanged
- The `seal_artifact` and `seal_env` helpers in `src/core/sync.rs` are unchanged
- `envy.enc` is byte-identical before and after a mismatch attempt (SC-003)

**Scale/Scope**: One behaviour change in one command; one function deletion; 9 new unit tests + 1 existing test updated; 1 new E2E scenario; 2 doc files updated; 1 version bump.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Security by Default** | ✓ Pass | The new behaviour closes a real security gap: a typo in `envy encrypt` (headless mode) can no longer silently change the envelope's passphrase. The strict-fail block verifies the passphrase before any modification. The error message contains no passphrase data. |
| **II. Determinism** | ✓ Pass | The behaviour is deterministic given the same inputs. The verify call uses `check_envelope_passphrase` which is pure (it calls `unseal_envelope`, which is deterministic). Exit codes are stable. |
| **III. Rust Best Practices** | ✓ Pass | `Result<T, E>` with typed errors throughout. No new `.unwrap()` or `.expect()` introduced. Unit tests for the new behaviour (9 new tests). Compiles with zero warnings under `cargo build` and passes `cargo clippy -- -D warnings`. |
| **IV. Modularity (4-layer)** | ✓ Pass | The verify logic lives in `cmd_encrypt` (cli). The verify call uses `crate::core::check_envelope_passphrase` (core, existing). `cmd_encrypt` does NOT call `crypto::seal_envelope` directly (it goes through `core::seal_env` which is unchanged). |
| **V. Language** | ✓ Pass | All new identifiers, comments, error messages, and documentation in English. |

All gates pass. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/013-encrypt-strict/
├── plan.md              # This file
├── spec.md              # Feature spec (already written)
├── research.md          # Phase 0 output — 3 user clarifications resolved into 8 decisions
├── data-model.md        # Phase 1 output — entity analysis + state transitions + memory lifecycle
├── quickstart.md        # Phase 1 output — step-by-step implementation guide
├── contracts/
│   ├── cli-encrypt.md   # CLI subcommand contract (replaces the old contract in cli-sync.md §2.1)
│   └── core-verify-reuse.md  # Internal Rust function reuse contract
├── checklists/
│   └── requirements.md  # Already created by /speckit.specify
└── tasks.md             # Phase 2 output (created by /speckit.tasks — NOT by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── cli/
│   ├── mod.rs           # No changes
│   ├── commands.rs      # Replace strict-verify block in cmd_encrypt; DELETE confirm_key_rotation function
│   ├── error.rs         # No changes
│   └── format.rs        # No changes
├── core/
│   ├── mod.rs           # No changes
│   ├── sync.rs          # No changes (reuse check_envelope_passphrase)
│   └── ...              # No changes
├── crypto/
│   └── ...              # No changes
└── db/
    └── ...              # No changes

tests/
├── cli_integration.rs   # No changes (ignored in CI anyway)
├── sync_artifact.rs     # No changes
├── e2e_devops_scenarios.sh  # Add Scenario 11
└── db/
    └── ...              # No changes

docs/
└── developer-guide.md   # Add paragraph to GitOps section

README.md                # Update command table + add paragraph in Multi-Env Passphrases section

Cargo.toml               # Bump version 0.3.0 → 0.3.1
```

**Structure Decision**: Single project (Option 1) — envy is a single Rust binary; this feature fits within the existing module structure with no new modules or sub-crates.

## Key Files & Functions

### `src/cli/commands.rs::cmd_encrypt` — the strict-verify block

The current code (lines 748-763) runs the pre-flight rotation check ONLY in interactive mode and asks for confirmation on mismatch. The new code (one block, ~10 lines) runs the check UNCONDITIONALLY and returns `Err(CliError::PassphraseInput(...))` on mismatch.

The change is:
- Removed the `if !headless` outer guard.
- Replaced the `if !confirm_key_rotation(env_name)? { continue; }` fall-through with `return Err(...)`.
- The `continue` (skip the env) is replaced with `return` (fail the whole command) because the mismatch is a user-input error, not a per-env warning.

### `src/cli/commands.rs::confirm_key_rotation` — DELETE

The function and its 6-line doc comment (lines 551-570) are removed entirely. The `dialoguer::Confirm` import is also removed if it's no longer used elsewhere (verified by `grep -n "Confirm" src/cli/commands.rs`).

## Reuse Map

| Need | Reused helper | Location |
|------|---------------|----------|
| Verify the passphrase against the existing envelope | `check_envelope_passphrase` | `src/core/sync.rs:348-354` |
| Resolve the passphrase (interactive or env-var) | `resolve_passphrase_for_env` | `src/cli/commands.rs:412` |
| Detect headless mode | `is_headless_mode` | `src/cli/commands.rs:513` |
| Atomic write of `envy.enc` | `write_artifact_atomic` | `src/core/sync.rs:201` |
| Empty-vault guard pattern | inline copy of existing block | `src/cli/commands.rs:727-736` |
| Error formatting (`error: ` prefix) | `format_cli_error` | `src/cli/error.rs:86-88` |
| Error exit code (PassphraseInput → 2) | `cli_exit_code` | `src/cli/error.rs:135` |
| Diceware suggestion (new-env only) | `suggest_passphrase` | `src/crypto/diceware.rs` |
| Diceware banner (after seal) | `print_diceware_banner` | `src/cli/commands.rs:538` |
| Sync marker write (on success) | `seal_env` | `src/core/sync.rs:225` |
| SHA-256 (in tests) | `sha2::Sha256` | added in spec 012 as dev-dep |

## Test Strategy

### Unit tests in `src/cli/commands.rs` (9 new + 1 updated)

**New tests** (added after `encrypt_skips_empty_env_with_warning` around line 2289):

1. `encrypt_first_time_seal_interactive_succeeds` — US1 AS#1: pre-existing behaviour, verify still passes.
2. `encrypt_first_time_seal_headless_succeeds` — US1 AS#2: pre-existing behaviour, verify still passes.
3. `encrypt_update_seal_matching_passphrase_succeeds` — US2 AS#1: new explicit test for the matching update case.
4. `encrypt_update_seal_headless_matching_succeeds_and_byte_changes` — US2 AS#2: new test that asserts the envelope is re-created (new salt + nonce) on matching update.
5. `encrypt_update_seal_mismatch_interactive_fails_exit_2` — US3 AS#1: new test for the interactive mismatch case.
6. `encrypt_update_seal_mismatch_headless_fails_exit_2` — US3 AS#2: new test for the headless mismatch case.
7. `encrypt_update_seal_global_envy_passphrase_mismatch_fails` — US3 AS#3: new test for the global `ENVY_PASSPHRASE` case.
8. `encrypt_update_seal_empty_vault_skips_with_warning` — US5 AS#1: new test for the empty-vault guard on update.
9. `encrypt_mismatch_leaves_artifact_unchanged_sha256` — US3 AS#4 + SC-003: new test that snapshots SHA-256 before and after a mismatch attempt.

**Updated test** (1 line at `src/cli/commands.rs:2084-2093`):
- The assertion message and the comment are updated to remove the "key-rotation warning path" reference. The test logic (asserting that wrong passphrase returns `false` from `check_envelope_passphrase`) is unchanged.

### E2E scenario in `tests/e2e_devops_scenarios.sh` (Scenario 11)

Purely additive — no existing scenario is modified. The new scenario:
1. Initialises a fresh project.
2. Sets a secret.
3. Seals with passphrase A (headless).
4. Captures SHA-256 of `envy.enc`.
5. Attempts to seal again with passphrase B (headless, using `ENVY_PASSPHRASE=<B>`).
6. Asserts exit code 2.
7. Captures SHA-256 of `envy.enc` and asserts it is identical to step 4.
8. Asserts that the error message contains "passphrase does not match" and "envy rotate".

## Documentation Changes

| File | Change |
|------|--------|
| `README.md` | Update `envy encrypt` row in command table (line 309). Add a paragraph in the "Multi-Environment with Separate Passphrases" section (line 362) about the strict behaviour. |
| `docs/developer-guide.md` | Add a paragraph to the GitOps section (line 76) about the strict behaviour. The existing §7.5 on `envy rotate` is unchanged. |
| `Cargo.toml` | Bump version `0.3.0` → `0.3.1` (patch). |
| `.github/workflows/ci.yml` | No change — the existing quality gate + E2E job picks up the new test scenario automatically. |
| Commit message | Document the behaviour change: `envy encrypt: passphrase mismatch with existing envelope now fails with exit 2 (was: silent rotation in headless mode). Use envy rotate to change the envelope's passphrase.` |

## Release Impact

- **Version**: 0.3.0 → 0.3.1 (PATCH per the spec's "Versioning" section).
- **cargo-dist release workflow**: picks up the new tag automatically on the next release. No workflow changes.
- **homebrew-tap**: no manual changes; the `publish-homebrew-formula` job generates the formula automatically.
- **npm**: no changes; the `@anguriatech/envy` wrapper just downloads the new binary.
- **envy-vscode**: out of scope for this spec; a follow-up spec will add the strict behaviour to the extension's `envy encrypt` calls.
- **smoke-test.yml**: the post-release test (which uses `envy encrypt` with a fresh vault) continues to work because the first-time-seal path is unchanged. No workflow changes.

## Out of Plan Scope (per spec's Out of Scope)

The following are explicitly NOT designed in this plan and are deferred to follow-up specs:

- Removing or modifying the `envy rotate` interactive prompt (preserved unchanged per spec 012).
- Adding a `--force-rotate` or `--allow-rotation` flag to `envy encrypt`.
- Changing `envy decrypt` behaviour.
- Modifying the VS Code extension.
- Spec 006 acceptance scenarios about "key rotation warning" in encrypt (the spec is unchanged; only the implementation in encrypt changed).

## Complexity Tracking

> No Constitution Check violations to justify. The 4-layer rule, the security-by-default rule, the no-new-deps constraint, the no-new-error-variants constraint, and the no-new-core-helpers constraint are all honoured by the design as described above.
