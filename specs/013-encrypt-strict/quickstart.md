# Quickstart: Implementing Strict `envy encrypt`

**Feature**: 013-encrypt-strict
**Date**: 2026-06-10

This is a step-by-step implementation guide for the developer picking up the task. Read this before opening the codebase.

## Pre-flight

1. Make sure you are on the `013-encrypt-strict` branch (created by the `before_specify` hook):
   ```bash
   git branch --show-current
   # Expected: 013-encrypt-strict
   ```
2. Read the spec: `specs/013-encrypt-strict/spec.md`.
3. Read the research: `specs/013-encrypt-strict/research.md`.
4. Read the data model: `specs/013-encrypt-strict/data-model.md`.
5. Read the contracts: `specs/013-encrypt-strict/contracts/cli-encrypt.md` and `contracts/core-verify-reuse.md`.

## Files to modify (in order)

1. `src/cli/commands.rs` — replace the `confirm_key_rotation` block in `cmd_encrypt` with the new strict block; delete the `confirm_key_rotation` function.
2. `src/cli/commands.rs` (tests module) — add 9 new unit tests; update 1 existing test (line 2084).
3. `tests/e2e_devops_scenarios.sh` — add Scenario 11 (or appended to S5).
4. `README.md` — update the `envy encrypt` row in the command table; add a paragraph in the "Multi-Environment with Separate Passphrases" section.
5. `docs/developer-guide.md` — add a paragraph to the GitOps section.
6. `Cargo.toml` — bump version 0.3.0 → 0.3.1.

## Implementation order

### Step 1: `src/cli/commands.rs` — replace the strict-verify block in `cmd_encrypt`

Open `src/cli/commands.rs`. Locate the per-env loop in `cmd_encrypt` (around line 715). The existing code is:

```rust
// T024: Pre-flight key-rotation check (interactive path only, FR-008, SC-004).
// Headless mode bypasses this check — CI operators know their passphrases.
if !headless {
    if let Some(existing_envelope) = artifact.environments.get(env_name) {
        if !crate::core::check_envelope_passphrase(
            passphrase.as_ref(),
            env_name,
            existing_envelope,
        ) {
            // Passphrase mismatch → warn and require explicit confirmation.
            if !confirm_key_rotation(env_name)? {
                continue; // User said No (or pressed Enter) → skip this env.
            }
            // User explicitly confirmed → fall through to seal.
        }
    }
}
```

Replace it with:

```rust
// v0.3.1+ (spec 013): strict verify-or-fail. Runs in BOTH headless and
// interactive mode. On mismatch, fail with exit 2 and direct the user
// to `envy rotate`. The old `confirm_key_rotation` flow is removed
// (see removed function below).
if let Some(existing_envelope) = artifact.environments.get(env_name) {
    if !crate::core::check_envelope_passphrase(
        passphrase.as_ref(),
        env_name,
        existing_envelope,
    ) {
        return Err(CliError::PassphraseInput(format!(
            "passphrase does not match the existing envelope.\nhint: use `envy rotate -e ENV` to change the envelope's passphrase."
        )));
    }
}
```

The change is:
- Removed the `if !headless` outer guard — the block now runs in both modes.
- Replaced the `if !confirm_key_rotation(env_name)? { continue; }` fall-through with an early `return Err(...)`.
- The `continue` (skip the env) is replaced with `return` (fail the whole command) because the mismatch is a user-input error, not a per-env warning.

### Step 2: `src/cli/commands.rs` — delete `confirm_key_rotation`

Open `src/cli/commands.rs`. The function is at lines 551-570. Delete the entire function definition plus its 6-line doc comment. The exact block to remove is:

```rust
/// Prompts the user to confirm a passphrase key-rotation for `env_name`.
///
/// Displays a high-visibility warning, then uses `Confirm` with `default(false)`
/// so pressing Enter or typing 'N' aborts the rotation (FR-008, SC-004).
///
/// Returns `Ok(true)` if the user explicitly confirms, `Ok(false)` to abort.
fn confirm_key_rotation(env_name: &str) -> Result<bool, CliError> {
    eprintln!(
        "\n  {} Passphrase does not match existing data for '{env_name}'.\n  \
         Continuing will ROTATE the key. Data sealed with the old passphrase\n  \
         will not be recoverable without it.\n",
        dialoguer::console::style("WARNING:").yellow().bold()
    );
    let theme = dialoguer::theme::ColorfulTheme::default();
    dialoguer::Confirm::with_theme(&theme)
        .with_prompt(format!("Rotate the key for '{env_name}'?"))
        .default(false)
        .interact()
        .map_err(|e| CliError::PassphraseInput(e.to_string()))
}
```

After deletion, verify with `grep -r confirm_key_rotation src/` — should return zero matches.

Also check the imports at the top of `src/cli/commands.rs`. If `dialoguer::Confirm` is no longer used anywhere in the file, remove the import. (A quick `grep -n "Confirm" src/cli/commands.rs` will confirm.)

### Step 3: `src/cli/commands.rs` — update the existing test (line 2084)

The test at `src/cli/commands.rs:2084-2093` has an assertion that references the "key-rotation warning path":

```rust
// Wrong passphrase → false (rotation warning path would trigger).
assert!(
    !crate::core::check_envelope_passphrase("pass-B", "development", envelope),
    "wrong passphrase must return false (key-rotation warning path)"
);
```

Update the comment and assertion message to remove the "key-rotation warning path" reference. The test logic (asserting that wrong passphrase returns `false`) is unchanged — the helper is still correct.

### Step 4: `src/cli/commands.rs` — add 9 new unit tests

Add the 9 new tests listed in the user's "Tests strategy" section, after the existing `encrypt_skips_empty_env_with_warning` test (around line 2289). The tests use the existing `TEST_MASTER_KEY`, `ENV_LOCK`, and `seal_test_env` helpers.

Test names (all in the `mod tests` block):
1. `encrypt_first_time_seal_interactive_succeeds` — US1 AS#1
2. `encrypt_first_time_seal_headless_succeeds` — US1 AS#2
3. `encrypt_update_seal_matching_passphrase_succeeds` — US2 AS#1
4. `encrypt_update_seal_headless_matching_succeeds_and_byte_changes` — US2 AS#2
5. `encrypt_update_seal_mismatch_interactive_fails_exit_2` — US3 AS#1
6. `encrypt_update_seal_mismatch_headless_fails_exit_2` — US3 AS#2
7. `encrypt_update_seal_global_envy_passphrase_mismatch_fails` — US3 AS#3
8. `encrypt_update_seal_empty_vault_skips_with_warning` — US5 AS#1
9. `encrypt_mismatch_leaves_artifact_unchanged_sha256` — US3 AS#4 + SC-003

Note: tests #1 and #2 verify existing behaviour; they should pass without changes to the production code (the first-time-seal path is unchanged).

### Step 5: `tests/e2e_devops_scenarios.sh` — add Scenario 11

Add a new section "Scenario 11 — Strict `envy encrypt`" that:
1. Initialises a fresh project.
2. Sets a secret.
3. Seals with passphrase A (headless).
4. Captures SHA-256 of `envy.enc`.
5. Attempts to seal again with passphrase B (headless, using `ENVY_PASSPHRASE=<B>`).
6. Asserts exit code 2.
7. Captures SHA-256 of `envy.enc` and asserts it is identical to step 4.
8. Asserts that the error message contains "passphrase does not match" and "envy rotate".

This is purely additive — no existing scenario is modified.

### Step 6: `README.md` — update command table and add a paragraph

In the command table (around line 309), update the `envy encrypt` row to mention the strict behaviour. Example wording:

```markdown
| `envy encrypt [-e ENV]` | `enc` | Seal vault into `envy.enc` (strict: passphrase must match an existing envelope — use `envy rotate` to change it) |
```

In the "Multi-Environment with Separate Passphrases" section (around line 362), add a short paragraph:

```markdown
#### Strict `envy encrypt` — no silent key rotation

Since v0.3.1, `envy encrypt` is strict: the passphrase you provide must either match the existing envelope (re-seal) or be the first time you're creating the envelope. If neither condition holds, `envy encrypt` fails with `passphrase does not match the existing envelope. use envy rotate -e ENV to change the envelope's passphrase.` (exit 2) — the artifact is left unchanged.

Use `envy rotate -e ENV` as the dedicated path for key rotation. It verifies the current passphrase before accepting the new one, so a typo in either passphrase cannot silently lock the team out.
```

### Step 7: `docs/developer-guide.md` — add a paragraph

In the GitOps section (around line 76), add a paragraph:

```markdown
#### `envy encrypt` is strict since v0.3.1

`envy encrypt` no longer silently re-seals an envelope with a different passphrase. The contract is:

- If the envelope does not exist in `envy.enc` → create with the user-supplied passphrase.
- If the envelope exists AND the passphrase matches → re-seal with the same passphrase (a fresh salt + nonce are generated regardless).
- If the envelope exists AND the passphrase does NOT match → fail with exit 2 and the hint `use envy rotate -e ENV to change the envelope's passphrase.`

This closes the silent-key-rotation gap that was present in v0.3.0 (where a headless `envy encrypt` with a wrong passphrase would silently re-seal the envelope, locking the rest of the team out). The dedicated path for key rotation is `envy rotate` (spec 012).
```

### Step 8: `Cargo.toml` — bump version

Bump from `0.3.0` to `0.3.1`. Patch bump on the minor per the spec's "Versioning" section. No other changes to `Cargo.toml`.

## Verification

Run these commands locally before opening the PR:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
ENVY_BIN=$(pwd)/target/debug/envy bash tests/e2e_devops_scenarios.sh
grep -r confirm_key_rotation src/  # Must return zero matches
```

The CI quality gate runs all of the above. The grep step is the explicit SC-006 acceptance criterion.

## Layer check (Constitution Principle IV)

Verify before opening the PR:

- `src/cli/commands.rs::cmd_encrypt` MUST NOT import from `crate::crypto` directly. It calls `crate::core::check_envelope_passphrase` only.
- `src/core/sync.rs` MUST NOT be modified by this spec. `check_envelope_passphrase` is reused as-is.

## Security gate (Constitution Principle I)

Verify before opening the PR:

- The new strict block does not log or print the passphrase value. The error message contains only the literal text "passphrase does not match..." (no actual passphrase data).
- The `Zeroizing<String>` binding for the passphrase (from `resolve_passphrase_for_env`) is dropped at the end of the per-env loop iteration, well before the function returns.

## No-panic audit (Constitution Principle III)

Verify before opening the PR:

- The new strict block does not introduce any new `.unwrap()` or `.expect()` calls.
- The `dialoguer::Confirm` import (if removed) does not break any other tests or code paths.
