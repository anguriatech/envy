# envy decrypt

Unseal `envy.enc` and restore secrets into the local vault.

## What it does

Reads the committed `envy.enc` artifact and upserts every successfully
decrypted environment into the local vault — the command developers run after
`git pull` to sync team secrets. Environments that cannot be decrypted with
the provided passphrase are listed as **skipped** (not an error), enabling
progressive disclosure: a developer holding only the dev key imports
`development`/`staging` and gracefully skips `production`.

## Aliases

| Alias | Notes |
|-------|-------|
| `dec` | Short alias |

## Syntax & flags

```text
envy decrypt
envy dec
```

No flags. The passphrase is prompted interactively, or read headlessly from
`ENVY_PASSPHRASE` / `ENVY_PASSPHRASE_<ENV>`.

## Examples

```bash
git pull
envy decrypt

# Headless (CI runner, using a repo secret)
ENVY_PASSPHRASE=${{ secrets.ENVY_KEY }} envy decrypt
```

## How it works

Unseals each envelope with its passphrase-derived key (Argon2id + AES-256-GCM)
and upserts the secrets into the vault. Failed envelopes are skipped and
reported — the command only exits non-zero if **zero** environments were
imported. Sync markers are updated so `envy status` reflects the restored
state.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (≥ 1 environment imported) |
| `1` | Zero environments imported |
| `2` | Wrong passphrase / missing input |
| `5` | `envy.enc` missing or unreadable |

## Related commands

- [envy encrypt](envy-encrypt.md) — the other side of the sync loop
- [envy status](envy-status.md) — verify sync state after restore
- [envy run](envy-run.md) — use the restored secrets
