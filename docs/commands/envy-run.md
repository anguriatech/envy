# envy run

Inject secrets as environment variables and run a child process.

## What it does

Fetches all secrets for the selected environment, injects them into the
child process environment, and proxies the child's exit code exactly. This is
the day-to-day command for running apps, scripts, and deployments with secrets
that never touch plaintext files:

```bash
envy run -- npm run dev
envy run -e production -- ./deploy.sh
```

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy run [-e ENV] -- COMMAND [ARGS...]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |
| `--` | Separator; everything after it is the child command |

## Examples

```bash
envy run -- npm run dev
envy run -e staging -- python main.py
envy run -e production -- ./server --port 8080
```

## How it works

Secrets are decrypted in memory (inside `Zeroizing` containers), injected
**in addition to** the inherited environment (never replacing it), and the
child process is spawned via `std::process::Command`. The exit code of the
child is proxied exactly. If the child is killed by a signal, `envy run`
exits `1`; if the binary cannot be executed, it exits `127` (like the shell).
`get`/`set`/`rm` actions performed by the child are recorded in the audit log.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Child exited successfully |
| `127` | Child binary not found |
| `N` | Child process exit code, proxied exactly |
| `4` | Vault or keyring failure (no child started) |

## Related commands

- [envy set](envy-set.md) — store the secrets `run` injects
- [envy status](envy-status.md) — check vault ↔ `envy.enc` sync first
- [envy scan](envy-scan.md) — ensure no plaintext copies exist before running
