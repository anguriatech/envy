# Data Model: Envelope Passphrase Rotation

**Feature**: 012-cli-rotate
**Date**: 2026-06-10

## Entities

This feature does not introduce new persistent entities. It operates on two existing entities and adds a behavioural contract to one of them.

### Sealed Envelope (existing, augmented with a re-seal operation)

| Field | Type | Notes |
|-------|------|-------|
| `ciphertext` | `Vec<u8>` (Base64) | AES-256-GCM ciphertext. Regenerated on rotation. |
| `nonce` | `[u8; 12]` (Base64) | Fresh random nonce from `OsRng`. Regenerated on rotation. |
| `kdf` | `KdfParams` | Argon2id parameters (`memory_kib`, `time_cost`, `parallelism`, `salt`). Salt is regenerated on rotation. |

**Behaviour added by this feature**: a `Sealed Envelope` can be re-sealed via the new `core::sync::rotate_env` function. The re-seal operation is **atomic from the artifact's perspective**: the envelope is either fully replaced (new ciphertext + nonce + KDF salt) or left untouched.

**Invariants preserved across rotation**:
- The plaintext payload (the set of key-value pairs) is unchanged — sourced from the local vault.
- The artifact structure (other envelopes) is unchanged.

### Artifact (`envy.enc`) (existing)

A `BTreeMap<String, EncryptedEnvelope>` keyed by environment name. The rotation operation:

1. Verifies the `current_passphrase` against the target envelope via `check_envelope_passphrase`.
2. Reads the secrets for `env_name` from the vault.
3. Re-seals via `seal_env` (which uses a fresh `OsRng` nonce and salt).
4. Replaces the envelope in the `BTreeMap` and writes the artifact atomically.

**Invariants preserved**:
- All other envelopes in the artifact are byte-identical before and after a rotation.
- The artifact's `version` field is unchanged.
- The artifact is either fully replaced (post-rotation) or fully untouched (pre-rotation) — never partial.

### Vault (existing, not modified)

The local vault is the **source of truth** for secret values. The `envy rotate` command MUST NOT modify the vault; it only re-seals the envelope in `envy.enc`. The vault's state is the input to the re-seal, and the new envelope contains the same plaintext as the old one.

**`sync_marker` side-effect**: `seal_env` (which we reuse for the re-seal) writes a fresh `sync_marker` row for the rotated env. This is intentional — it tells `envy status` to report the env as `InSync` immediately after the rotation. The vault's `secrets` rows are not touched.

## State Transitions

```
                          ┌──────────────────────┐
                          │ Envelope exists in   │
                          │ envy.enc, sealed     │
                          │ with passphrase A    │
                          └──────────┬───────────┘
                                     │
                                     │ envy rotate -e ENV
                                     │ (user enters A, then B twice)
                                     ▼
                          ┌──────────────────────┐
                          │ Envelope re-sealed   │
                          │ with passphrase B    │
                          │ (ciphertext, nonce,  │
                          │  KDF salt all fresh) │
                          └──────────────────────┘
```

**Failure transitions** (any of these leaves the envelope in its original state):
- Wrong current passphrase → envelope unchanged, CLI exits with code 2.
- New passphrase equals current → envelope unchanged, CLI exits with code 2.
- Confirmation mismatch → envelope unchanged, CLI exits with code 2.
- Whitespace-only new passphrase → envelope unchanged, CLI exits with code 2.
- No TTY and no env vars → envelope unchanged, CLI exits with code 2.
- Missing `envy.enc` → CLI exits with code 1.
- Missing envelope in artifact → CLI exits with code 3.
- Local vault has 0 secrets for the env → envelope unchanged, warning printed, CLI exits with code 0.

## Memory Lifecycle

Per Constitution Principle I, the new and current passphrases must be zeroed as early as possible. The lifecycle is:

```
  dialoguer::Password::interact()
          │
          │ returns String (raw)
          ▼
  Zeroizing<String>::new(raw)        ← wrapped immediately at the call site
          │
          │ passed to core::sync::rotate_env as &str (no copy)
          ▼
  core::sync::rotate_env:
      check_envelope_passphrase(&str)  ← view only
      seal_env(&str)                   ← view only
          │
          │ core::sync::rotate_env returns
          ▼
  <binding goes out of scope>
          │
          │ Zeroizing<String>::drop() is called
          ▼
  Memory is zeroed via ptr::write_bytes(0, len)
```

The `Zeroizing<String>` binding lives in the `cmd_rotate` function scope and is dropped before the function returns (or before any early `return` statement — the binding is in scope at the return point and is dropped before the return value is computed).
