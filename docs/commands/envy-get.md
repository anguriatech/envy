# envy get

Print the decrypted value of a secret.

## What it does

Outputs the raw decrypted value of a single secret — no labels, no trailing
metadata — so the output is pipeline-safe:

```bash
envy get API_KEY | pbcopy
```

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy get KEY [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |

## Examples

```bash
envy get STRIPE_KEY
envy get DB_URL -e production
envy get API_TOKEN | pbcopy   # copy to clipboard without echo
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Decrypts the requested row (AES-256-GCM) and writes the value to stdout as-is.
The value is decrypted in memory inside a `Zeroizing` container and dropped as
soon as possible. Because the output is a raw value, `envy get` is safe for
shell pipelines and script substitution.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Key not found in the environment |
| `2` | Empty or invalid key name |
| `4` | Vault or crypto failure |

## Related commands

- [envy set](envy-set.md) — store a value first
- [envy export](envy-export.md) — print all secrets at once
- [envy list](envy-list.md) — list key names without values
