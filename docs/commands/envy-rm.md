# envy rm

Delete a secret.

## What it does

Removes a secret from the vault for the target environment. Deleting a
secret locally does not modify `envy.enc` — run `envy encrypt` afterwards to
propagate the deletion to the artifact.

## Aliases

| Alias | Notes |
|-------|-------|
| `remove` | Long alias |
| `unset` | Long alias |

## Syntax & flags

```text
envy rm KEY [-e ENV]
envy remove KEY [-e ENV]
envy unset KEY [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |

## Examples

```bash
envy rm STRIPE_KEY
envy rm DB_URL -e production
```

## How it works

Removes the encrypted row from the vault. Deletion is immediate and local;
the sync marker is updated so `envy status` reports drift until you run
`envy encrypt` to re-seal `envy.enc`. No plaintext is ever written — the
row is dropped from the encrypted database.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Key not found in the environment |
| `4` | Vault or crypto failure |

## Related commands

- [envy set](envy-set.md) — add or update a secret
- [envy encrypt](envy-encrypt.md) — propagate the deletion to `envy.enc`
- [envy diff](envy-diff.md) — preview vault vs artifact drift
