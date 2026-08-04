# envy status

Show the sync status of all vault environments.

## What it does

Displays a dashboard of environments with secret count, last-modified time,
and sync state relative to `envy.enc` — plus a rotation reminder when an
envelope's passphrase is older than the configured `rotation_reminder_days`.
Read-only: never prompts for a passphrase and never decrypts values. Use
`--format json` for machine-readable CI gates.

## Aliases

| Alias | Notes |
|-------|-------|
| `st` | Short alias |

## Syntax & flags

```text
envy status
envy st [-f FORMAT]
```

| Flag | Description |
|------|-------------|
| `-f, --format <FORMAT>` | `table` (default), `json` — global flag |

## Examples

```bash
envy status
envy status --format json | jq '.environments[] | select(.status == "in_sync")'
```

## How it works

Reads vault rows, `envy.enc` presence, and the V2 `sync_markers` table to
derive per-environment status (`in_sync` / drift / missing artifact). It also
compares each envelope's sealed-at timestamp against `rotation_reminder_days`
from `envy.toml` (default 90; `0` disables the reminder). No secrets are
touched — the JSON output is safe to parse in CI/CD pipelines.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (regardless of sync state) |
| `4` | Vault failure |

## Related commands

- [envy encrypt](envy-encrypt.md) / [envy decrypt](envy-decrypt.md) — resolve drift
- [envy diff](envy-diff.md) — see the actual key-level differences
- [envy rotate](envy-rotate.md) — act on the rotation reminder
