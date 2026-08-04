# envy set

Store or update a secret.

## What it does

Adds a new secret or updates an existing one for the target environment.
Use it when you need to introduce a credential into the vault — locally or
in CI — before `envy encrypt` seals it into the `envy.enc` artifact.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy set KEY=VALUE [-e ENV]
envy set --stdin KEY [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |
| `--stdin` | Read the value from standard input (never appears in process listings or shell history) |

## Examples

```bash
# Inline assignment — only the FIRST '=' separates key from value
envy set STRIPE_KEY=sk_test_dummy123

# Multi-environment
envy set DB_URL=postgres://prod-db -e production

# Secrets from stdin (safe against ps/history leaks)
echo "super-secret-value" | envy set --stdin API_TOKEN
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

The value is encrypted with AES-256-GCM and stored per-row inside the
SQLCipher-encrypted vault. It is held in a `Zeroizing` container in memory and
never written to plaintext files. Reading from stdin avoids the value
appearing in `ps`, `/proc/<pid>/environ`, or shell history. In headless
environments, the vault key resolves via the OS keyring (or the deterministic
fallback when `ENVY_PASSPHRASE` is exported).

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Vault not initialised / project not found |
| `2` | Invalid assignment format (no `=`) |
| `4` | Vault or crypto failure |

## Related commands

- [envy get](envy-get.md) — read a value back
- [envy list](envy-list.md) — see all key names
- [envy encrypt](envy-encrypt.md) — seal the vault into `envy.enc`
