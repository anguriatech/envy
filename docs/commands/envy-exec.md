# envy exec

Resolve the real binary outside the shims dir and run it (hidden plumbing).

## What it does

This is what every file in `~/.envy/shims/` calls — not something humans run
by hand (use shims or `envy run` instead, which is why it's hidden from help).
In an opted-in project it injects vault secrets scoped to the child process,
exactly like `envy run` (same exit-code proxying, same audit entry).
Elsewhere it spawns the real binary transparently without touching the vault.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — (hidden from `--help`) |

## Syntax & flags

```text
envy exec [-e ENV] -- COMMAND [ARGS...]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `$ENVY_ENV` or `development`) |
| `--` | Separator; everything after it is the real command |

## Examples

```bash
# What ~/.envy/shims/npm effectively does:
envy exec -- npm run dev
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Strips the shims dir from `PATH` before resolving (so a shim can never
recurse into itself — including npm-spawns-npx chains), walks up for
`envy.toml` with `auto_inject`, and then either delegates to the `run`
implementation (one shared spawn/audit/exit-code path) or spawns directly on
the fast path. Explicit paths (`./x`, `/bin/y`) bypass lookup by definition.
Vault or keyring failures are loud (stderr, exit 4) — a shim never runs naked
silently in an opted-in project. On Windows, `.cmd`/`.bat` targets route
through `cmd.exe /D /C` (`CreateProcess` cannot execute batch files directly);
parameters with cmd metacharacters (`&|^%`) keep cmd's native quoting quirks,
same as npm scripts on Windows.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Child exited successfully |
| `127` | Child binary not found |
| `N` | Child process exit code, proxied exactly |
| `1` | Child killed by a signal, or cannot determine current directory |
| `4` | Vault or keyring failure (no child started) |

## Related commands

- [envy reshim](envy-reshim.md) — generate the shims that call this
- [envy run](envy-run.md) — the human-facing scoped runner (same guarantees)
- [envy doctor](envy-doctor.md) — verify resolution works
