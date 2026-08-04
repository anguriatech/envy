# envy encrypt

Seal the local vault into the encrypted `envy.enc` GitOps artifact.

## What it does

Encrypts the vault's environments into a single `envy.enc` file that is safe
to commit to Git. The artifact becomes the source of truth for team sync:
colleagues run `envy decrypt` after `git pull` to restore secrets. All
environments are sealed by default; use `-e` to seal one.

Strict mode: `envy encrypt` refuses to silently rotate an existing envelope's
passphrase — if the passphrase does not match an existing envelope, it errors.
Use `envy rotate` to change a passphrase.

## Aliases

| Alias | Notes |
|-------|-------|
| `enc` | Short alias |

## Syntax & flags

```text
envy encrypt [-e ENV]
envy enc [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Seal only this environment (default: all) |

Interactive mode prompts for a passphrase with confirmation. Headless CI mode
reads `ENVY_PASSPHRASE_<ENV>` (e.g. `ENVY_PASSPHRASE_DEVELOPMENT`).

## Examples

```bash
envy encrypt
envy enc -e staging
ENVY_PASSPHRASE_DEVELOPMENT='team-pass-dummy' envy encrypt -e development < /dev/null
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Each environment is sealed with its own passphrase-derived key
(Argon2id KDF + AES-256-GCM), all packed into the `envy.enc` JSON artifact.
In multi-key mode, environments locked with different passphrases stay
independently decryptable — least-privilege access by design. After sealing,
sync markers are updated so `envy status` reports `in_sync`.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | No environments/secrets to seal |
| `2` | Passphrase mismatch (strict mode) or empty input |
| `4` | Vault or crypto failure |
| `5` | Existing `envy.enc` unreadable |

## Related commands

- [envy decrypt](envy-decrypt.md) — the other side of the sync loop
- [envy diff](envy-diff.md) — preview what will change before sealing
- [envy rotate](envy-rotate.md) — change an envelope passphrase safely
- [envy status](envy-status.md) — verify sync state
