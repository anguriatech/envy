# envy list

List all secret key names for the environment.

## What it does

Prints key names one per line in alphabetical order. By default the `table`
format shows **only key names — never values**. Be careful: `--format json`,
`--format dotenv`, or `--format shell` decrypt and reveal the actual values.

## Aliases

| Alias | Notes |
|-------|-------|
| `ls` | Short alias |

## Syntax & flags

```text
envy list [-e ENV]
envy ls [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |
| `-f, --format <FORMAT>` | `table` (default), `json`, `dotenv`, `shell` — global flag |

## Examples

```bash
envy list
envy ls -e staging

# JSON for scripts (WARNING: includes decrypted values)
envy list --format json
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Reads the environment's secret rows from the encrypted vault and prints only
the key names in the default table format, so no decryption of values is
needed. Switching to `json`/`dotenv`/`shell` formats decrypts every value and
emits it to stdout — treat that output as sensitive.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (even if the environment has no secrets) |
| `4` | Vault or crypto failure |

## Related commands

- [envy get](envy-get.md) — print a single value
- [envy export](envy-export.md) — print all values in a chosen format
- [envy set](envy-set.md) — add secrets
