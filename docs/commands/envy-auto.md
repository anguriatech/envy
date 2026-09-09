# envy auto

Manage transparent shell auto-injection for this project (no more `envy run --` prefix).

## What it does

`envy auto on` opts the current project into auto-injection (`auto_inject = true`
in `envy.toml`). Combined with the one-time `eval "$(envy shell-init <shell>)"`
setup, entering the project directory auto-exports vault secrets into your shell —
bare `npm run dev` just works, for humans and AI agents alike, with envy values
taking precedence over a legacy `.env` file (a warning is printed).

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy auto [on|off|status]
```

| Argument | Description |
|----------|-------------|
| `on` | Enable auto-injection for this project |
| `off` | Disable auto-injection for this project |
| `status` | Show whether auto-injection is on or off (default when omitted) |

## Examples

```bash
# Opt in and print the one-time shell setup
envy auto on

# Check state
envy auto status
#   auto-inject: on (my-project)

# Opt out again (cd out and back to unload already-exported keys)
envy auto off
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

`auto` only flips the `auto_inject` flag in `envy.toml` (text edit — your
`rotation_reminder_days` and comments are preserved). The actual injection is done
by the shell hook ([envy shell-init](envy-shell-init.md) + [envy hook](envy-hook.md)):
environment selection is `$ENVY_ENV` or `development`, and `ENVY_AUTO_INJECT=0`
disables injection globally.

Security tradeoff: unlike the scoped `envy run` (secrets live only in one child
process), auto-inject exports secrets into your interactive shell, visible to every
child process. Prefer `envy run` in CI and for production deploys.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Manifest not found (run `envy init` first) |
| `4` | Vault failure |

## Related commands

- [envy shell-init](envy-shell-init.md) — one-time shell setup the hook needs
- [envy hook](envy-hook.md) — what the shell hook calls on every `cd`
- [envy run](envy-run.md) — scoped one-shot injection (better for CI/production)
- [envy init](envy-init.md) — `envy init --auto-inject` opts in from the start
