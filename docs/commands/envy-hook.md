# envy hook

Print `eval`-able exports for the current directory (shell prompt hook).

## What it does

Meant to be called by the shell hook installed via
[envy shell-init](envy-shell-init.md) — not by hand. Stdout carries shell code
(`eval "$(envy hook --shell bash)"`); stderr carries warnings. Entering a project
with `auto_inject = true` exports its vault secrets; leaving (or disabling via
`envy auto off`) unloads previously exported keys.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy hook [--shell bash|zsh|fish|powershell|nushell] [-e ENV]
```

| Flag | Description |
|------|-------------|
| `--shell <SHELL>` | Output syntax (default: `bash`; snippets always pass this explicitly) |
| `-e, --env <ENV>` | Target environment (default: `$ENVY_ENV` or `development`) |

## Examples

```bash
# What the hook evaluates on every prompt (bash/zsh):
eval "$(envy hook --shell bash)"

# Preview what would be injected right now:
envy hook --shell bash

# Another environment for this shell session:
ENVY_ENV=staging eval "$(envy hook --shell bash)"
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Resolution order per invocation: `--env` flag > `$ENVY_ENV` > `development`
(lowercased, like everywhere else). Exported variables live in the parent
environment, and every major dotenv loader (Node `dotenv`, Vite, Next.js, Django,
Rails) refuses to overwrite already-set variables — so envy wins over a legacy
`.env` file by construction, and a warning is printed to stderr when both exist.
Key names are validated before interpolation (vault keys and the `$__ENVY_KEYS`
tracking variable alike), values are single-quote escaped per shell, and
`--shell nushell` emits JSON applied via `load-env` instead of `eval`.

A prompt hook must never break the shell, so `envy hook` **always exits 0**:
outside a project, with auto-inject off, or with an unreachable vault/keyring it
prints unload-cleanup code or nothing at all — never an error. `ENVY_AUTO_INJECT=0`
forces unload-only mode globally.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Always — hook output is advisory, never an error |

## Related commands

- [envy shell-init](envy-shell-init.md) — installs the hook that calls this
- [envy auto](envy-auto.md) — opt a project in/out
- [envy run](envy-run.md) — scoped one-shot injection (better for CI/production)
- [envy scan](envy-scan.md) — confirm no plaintext `.env` copies remain
