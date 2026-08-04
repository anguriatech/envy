# envy audit

Show the local audit trail of secret-touching actions.

## What it does

Prints the history of `set`, `get`, `rm`, and `run` actions — key name and
timestamp only, never the secret value — from the local vault. Sync/crypto
actions (`encrypt`, `decrypt`, `rotate`) are not recorded here; their history
lives in `envy.enc`'s git log and `envy status`. The audit log is
best-effort: a recording failure never fails the underlying operation.

## Aliases

| Alias | Notes |
|-------|-------|
| `au` | Short alias |

## Syntax & flags

```text
envy audit [-e ENV] [--limit N]
envy au [-e ENV] [--limit N]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Restrict the report to one environment (default: all) |
| `--limit <N>` | Maximum entries, newest first (default: 50) |

## Examples

```bash
envy audit
envy audit -e production --limit 10
```

## How it works

Reads the `audit_logs` table from the encrypted vault. Recording happens
inside `audit_best_effort`: the action itself succeeds even if the log write
fails, so audit never blocks development. Only key names and timestamps are
persisted — values are never logged, even partially.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (even if the log is empty) |
| `4` | Vault failure |

## Related commands

- [envy status](envy-status.md) — sync + rotation state
- [envy hooks](envy-hooks.md) — prevent leaks at commit time
- [envy scan](envy-scan.md) — detect existing plaintext copies
