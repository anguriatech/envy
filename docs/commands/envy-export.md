# envy export

Print all secrets for an environment to stdout.

## What it does

Outputs every secret of an environment in a machine-readable format. The
default `dotenv` format (`KEY=value` one per line) is suitable for generating
`.env` files; `shell` format emits `export KEY='value'` safe for
`eval $(envy export --format shell)`; `json` is for scripts and CI.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy export [-e ENV] [-f FORMAT]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |
| `-f, --format <FORMAT>` | `dotenv` (default), `json`, `shell`, `table` — global flag |

## Examples

```bash
envy export                          # dotenv
envy export -e staging --format json
eval $(envy export --format shell)   # load into the current shell
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Decrypts all secrets of the environment in memory and serialises them to
stdout in the chosen format. The `shell` format single-quotes every value, so
it is safe to `eval` — the only shell hook envy ships. Because this command
reveals secret values, treat its output as sensitive.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `4` | Vault or crypto failure |

## Related commands

- [envy get](envy-get.md) — print a single secret
- [envy list](envy-list.md) — print key names only
- [envy run](envy-run.md) — inject secrets directly into a child process
