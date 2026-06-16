# CLI Contract: Strict `envy encrypt`

**Feature**: 013-encrypt-strict
**Date**: 2026-06-10
**Type**: CLI subcommand contract (replacement for the previous contract in `contracts/cli-sync.md` §2.1)

## Synopsis

```
envy encrypt [-e ENV]
envy enc [-e ENV]    # alias
```

## Description

Seals the local vault contents into `envy.enc` for one or all environments.

**Strict behaviour (v0.3.1+)**: the user-supplied passphrase MUST either:
- (a) match the existing envelope in `envy.enc` (the "update" case), or
- (b) be the first time an envelope is created for the env (the "new" case).

If neither condition holds (i.e. an envelope exists AND the passphrase does not match), the CLI MUST fail with exit 2 and direct the user to `envy rotate`.

**Use `envy rotate -e ENV` to change an envelope's passphrase.** This is the dedicated, safe path for key rotation (spec 012).

## Arguments

| Flag | Short | Required | Description |
|------|-------|----------|-------------|
| `--env` | `-e` | No | The environment to seal (default: all environments in the vault). |

## Environment Variables

| Variable | Used for | Required for headless mode? |
|----------|----------|-----------------------------|
| `ENVY_PASSPHRASE` | Global passphrase (no per-env suffix) | Yes (any of the headless paths) |
| `ENVY_PASSPHRASE_<ENV>` | Per-env passphrase | Yes (alternative to global) |

Notes:
- The global `ENVY_PASSPHRASE` is honoured ONLY in headless mode (no TTY). In interactive mode, the prompt takes precedence.
- A mismatch in headless mode fails with exit 2 (was: silent rotation in v0.3.0). This is the breaking change in v0.3.1.

## Interactive Prompts (TTY mode)

When no TTY is present and the required env vars are not set, the CLI exits with code 2 and a clear error.

When a TTY is present:

- **New-env case** (no envelope in `envy.enc` for the env): prompts for the new passphrase with double-entry confirmation. The existing Diceware suggestion flow continues to work.
- **Update-env case** (envelope exists): prompts for the CURRENT passphrase (one prompt, no double entry). Verifies against the existing envelope via `core::check_envelope_passphrase`. On match, re-seals. On mismatch, exits 2 with the hint below.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — at least one envelope sealed (or all empty envs skipped with warning). |
| 1 | File not found (only possible for nested errors, e.g. malformed `envy.enc`). |
| 2 | Invalid input: wrong passphrase (mismatch with existing envelope), confirmation mismatch, whitespace-only passphrase, no TTY and no env vars. |
| 3 | Initialisation conflict (env not in vault). |
| 4 | Vault or file write failure. |

## Error Messages

### Mismatch (Case: envelope exists, passphrase does not match)

The CLI prints to stderr:

```
error: passphrase input failed: passphrase does not match the existing envelope.
hint: use `envy rotate -e ENV` to change the envelope's passphrase.
```

Exit code: 2.

The `error: ` prefix is added by `format_cli_error` at `src/cli/error.rs:86-88`. The `passphrase input failed: ` text is added by the `Display` impl of `CliError::PassphraseInput` at `src/cli/error.rs:48-49`. The literal `\n` between the error sentence and the hint is embedded in the `String` payload of the variant.

### Empty vault (no secrets locally)

The CLI prints to stderr:

```
  ⚠  environment 'ENV' has 0 secrets, skipping
```

Exit code: 0. The artifact is unchanged.

## Behavioural Contract

1. The CLI MUST NOT silently re-seal an existing envelope with a different passphrase.
2. The CLI MUST verify the current passphrase by attempting to unseal the existing envelope before re-sealing.
3. On a mismatch, the CLI MUST exit 2 with the exact error message above.
4. On a mismatch, the CLI MUST NOT modify `envy.enc` (the atomic write helper is not even called).
5. On a mismatch, the CLI MUST NOT modify the local vault.
6. The empty-vault guard MUST apply in BOTH the new-env and update-env cases. A vault with 0 secrets for the env always results in a skip with warning, regardless of whether the envelope exists in `envy.enc`.
7. The Diceware suggestion flow for new envelopes is preserved unchanged.
8. The `sync_markers` table is updated on every successful seal (unchanged behaviour; the existing `seal_env` helper writes the marker).
9. The atomic write of `envy.enc` is used for every successful seal.
10. The global `ENVY_PASSPHRASE` env var is honoured in headless mode, but with the same strict verify-or-fail behaviour. This preserves backward compatibility with existing CI scripts.

## Examples

### First-time seal (interactive)

```bash
$ envy encrypt -e production
New passphrase for 'production':
Confirm passphrase for 'production':
Sealed 1 environment(s) → /path/to/envy.enc
  ✓  production
```

### Update seal with matching passphrase (interactive)

```bash
$ envy encrypt -e production
Passphrase for 'production':
Sealed 1 environment(s) → /path/to/envy.enc
  ✓  production
```

The envelope is re-sealed with a fresh salt + nonce but the same passphrase.

### Update seal with wrong passphrase (headless)

```bash
$ ENVY_PASSPHRASE_PRODUCTION=WRONG envy encrypt -e production
error: passphrase input failed: passphrase does not match the existing envelope.
hint: use `envy rotate -e ENV` to change the envelope's passphrase.
$ echo $?
2
```

`envy.enc` is byte-identical to its pre-attempt state.

### Empty vault (no secrets locally)

```bash
$ envy encrypt -e production
  ⚠  environment 'production' has 0 secrets, skipping
$ echo $?
0
```

## Out of Contract (deferred)

- Removing `envy rotate`'s interactive prompt (preserved per spec 012).
- Adding a `--force-rotate` flag to `envy encrypt` (users have `envy rotate`).
- Changing `envy decrypt` behaviour.
- Changes to the VS Code extension (a follow-up spec).
- Spec 006 acceptance scenarios about "key rotation warning" in encrypt (the spec is unchanged; only the implementation in encrypt changed).
