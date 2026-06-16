# Internal API Contract: `core::check_envelope_passphrase` (reused)

**Feature**: 013-encrypt-strict
**Date**: 2026-06-10
**Type**: Reuse contract — no new function; this spec reuses an existing helper.

## Signature

```rust
pub fn check_envelope_passphrase(
    passphrase: &str,
    env_name: &str,
    envelope: &EncryptedEnvelope,
) -> bool
```

**Location**: `src/core/sync.rs:348-354`.

## Behaviour

Returns `true` if `passphrase` successfully decrypts `envelope`, `false` otherwise.

Both wrong-passphrase and tampered-ciphertext cases return `false` — AES-GCM cannot distinguish them, and both warrant the strict-fail behaviour in `envy encrypt`.

## Reuse in `envy encrypt` (v0.3.1+)

In the per-env loop in `cmd_encrypt`, after the empty-vault guard and after `resolve_passphrase_for_env`, the new strict block calls this function:

```rust
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

This block runs unconditionally (BOTH headless and interactive mode) — the previous v0.3.0 behaviour was to run it only in interactive mode and to fall back to a silent rotation in headless mode.

## Why this signature

- `&str` for the passphrase (not `Zeroizing<String>`) because the core layer does not own secret-bearing state; the CLI layer wraps the passphrase in `Zeroizing` and passes a view. The `&str` view is constructed on demand and never outlives the `Zeroizing` binding in the caller's scope.
- `&EncryptedEnvelope` because the function is read-only.
- `bool` return type because the function is a simple "matches or not" check. The detailed failure mode (wrong-passphrase vs. tampered-ciphertext) is intentionally collapsed — both warrant the same response from `envy encrypt` (fail with exit 2).

## Out of Contract

- This spec does NOT modify `check_envelope_passphrase`. The helper is reused as-is.
- This spec does NOT add a new core helper for verification. The CLI layer is the right place for the strict-fail logic.
