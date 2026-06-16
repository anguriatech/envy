# Data Model: Strict `envy encrypt` (No Silent Key Rotation)

**Feature**: 013-encrypt-strict
**Date**: 2026-06-10

## Entities

This feature does not introduce new persistent entities. It tightens the contract of an existing command and modifies one transient relationship.

### Sealed Envelope (existing, contract-tightened)

| Field | Type | Notes |
|-------|------|-------|
| `ciphertext` | `Vec<u8>` (Base64) | AES-256-GCM ciphertext. Regenerated on every seal (matching or new). |
| `nonce` | `[u8; 12]` (Base64) | Fresh random nonce from `OsRng`. Regenerated on every seal. |
| `kdf` | `KdfParams` | Argon2id parameters (`memory_kib`, `time_cost`, `parallelism`, `salt`). Salt is regenerated on every seal. |

**Behaviour tightened by this feature**: the `envy encrypt` command's relationship to an existing envelope is now restricted to TWO cases:
- **Case A**: the envelope does not exist in `envy.enc` → create with the user-supplied passphrase.
- **Case B**: the envelope exists AND the user-supplied passphrase matches the existing envelope → re-seal with the same passphrase (fresh salt + nonce are generated because `seal_envelope` is called fresh).

The third case (envelope exists AND passphrase does NOT match) is no longer reachable in `envy encrypt`. The CLI MUST fail with `PassphraseInput` and exit 2. The user is directed to `envy rotate` (spec 012) for the rotation use case.

**Invariants preserved across tightening**:
- The plaintext payload (the set of key-value pairs) is unchanged across a re-seal — sourced from the local vault.
- The artifact structure (other envelopes) is unchanged.
- The atomic write of `envy.enc` is unchanged.
- The `sync_markers` table update on success is unchanged.
- The Diceware suggestion + banner flow for new envelopes is unchanged.

### Passphrase env vars (existing, behaviour-preserved)

| Variable | Used in | Behaviour before | Behaviour after |
|----------|---------|------------------|-----------------|
| `ENVY_PASSPHRASE_<ENV>` | per-env headless | matches → seal; mismatches → silent rotation | matches → seal; mismatches → fail with exit 2 |
| `ENVY_PASSPHRASE` (global) | headless | matches → seal; mismatches → silent rotation | matches → seal; mismatches → fail with exit 2 |
| TTY prompt | interactive | matches → seal; mismatches → warn + ask confirmation | matches → seal; mismatches → fail with exit 2 |

The env-var names are unchanged. The only behaviour change is what happens on a mismatch.

### Empty-vault guard (existing, position-tightened)

| Condition | Old behaviour | New behaviour |
|-----------|---------------|---------------|
| New-env case + 0 secrets locally | skip with warning, exit 0 | unchanged |
| Update-env case + 0 secrets locally | seal an empty envelope over the existing one | **skip with warning, exit 0** (consistent with new-env case) |

The guard is unchanged in code (the existing `if secret_keys.is_empty()` block at `src/cli/commands.rs:727-736`). What changes is the order: the guard now runs BEFORE the verify step, so even users running with the correct passphrase get the "nothing to seal" warning.

## State Transitions

```
                          ┌──────────────────────┐
                          │ Envelope exists in   │
                          │ envy.enc, sealed     │
                          │ with passphrase A    │
                          └──────────┬───────────┘
                                     │
                                     │ envy encrypt -e ENV
                                     │ (user types B — wrong)
                                     ▼
                          ┌──────────────────────┐
                          │ envy.enc UNCHANGED   │
                          │ vault UNCHANGED      │
                          │ CLI exits 2 with:    │
                          │ error: passphrase    │
                          │ input failed:        │
                          │ passphrase does not  │
                          │ match the existing   │
                          │ envelope.            │
                          │ hint: use `envy      │
                          │ rotate -e ENV` ...   │
                          └──────────────────────┘

                          ┌──────────────────────┐
                          │ Envelope exists in   │
                          │ envy.enc, sealed     │
                          │ with passphrase A    │
                          └──────────┬───────────┘
                                     │
                                     │ envy encrypt -e ENV
                                     │ (user types A — correct)
                                     ▼
                          ┌──────────────────────┐
                          │ Envelope re-sealed   │
                          │ with passphrase A    │
                          │ (fresh salt + nonce) │
                          │ CLI exits 0          │
                          └──────────────────────┘
```

## Memory Lifecycle

Per Constitution Principle I, the passphrases must be zeroed as early as possible. The existing `resolve_passphrase_for_env` helper returns `Zeroizing<String>`, which the new verify block passes by `&str` reference to `check_envelope_passphrase`. The `Zeroizing<String>` binding is dropped at the end of the per-env loop iteration, well before the function returns.

The new `CliError::PassphraseInput(format!(...))` does NOT contain any passphrase data — it contains the literal error message text. The `format!` call does not move or copy the passphrase.

## Failure Transitions (any of these leaves `envy.enc` byte-identical)

- Wrong current passphrase → envelope unchanged, CLI exits 2 with the hint
- Whitespace-only passphrase → envelope unchanged, CLI exits 2 (existing behaviour)
- Confirmation mismatch (new-env case) → envelope unchanged, CLI exits 2 (existing behaviour)
- No TTY and no env vars → envelope unchanged, CLI exits 2 (existing behaviour)
- Missing `envy.enc` → treated as first-time seal for every requested env (existing behaviour)
- Empty vault (0 secrets locally) → envelope unchanged, warning printed, CLI exits 0 (NEW consistency rule for the update case)
