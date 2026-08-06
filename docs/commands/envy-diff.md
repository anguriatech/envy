# envy diff

Compare local vault secrets against the sealed `envy.enc` artifact.

## What it does

Produces a Git-style diff of additions, deletions, and modifications between
the vault and the artifact for one environment — the pre-encrypt review loop.
By default only key names are shown; `--reveal` includes values (with a
stderr warning). Exit code 0 = clean, 1 = differences found, 2+ = error, so
it is safe to gate scripts with `envy diff ||` without masking real errors.

## Aliases

| Alias | Notes |
|-------|-------|
| `df` | Short alias |

## Syntax & flags

```text
envy diff [-e ENV] [--reveal] [-f FORMAT]
envy df [-e ENV] [--reveal]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |
| `--reveal` | Show decrypted values (stderr warning emitted first) |
| `-f, --format <FORMAT>` | `table` (default), `json` — global flag |

## Examples

```bash
envy diff
envy diff -e production --reveal
envy diff --format json           # old_value/new_value absent without --reveal
```

## How it works

Fetches vault secrets and unseals the artifact's envelope for the environment
(only if it exists), then runs a pure key-level comparison: added (vault
only), removed (artifact only), modified (value differs). Unchanged keys are
dropped immediately; values live in `Zeroizing` containers. In `json` format
`old_value`/`new_value` are included **only** with `--reveal`.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | No differences |
| `1` | Differences found (diff(1) convention) |
| `2+` | Error (wrong passphrase, unreadable artifact, missing env) |

## Related commands

- [envy encrypt](envy-encrypt.md) — the operation `diff` previews
- [envy status](envy-status.md) — per-environment sync dashboard
- [envy rotate](envy-rotate.md) — fix a passphrase mismatch
