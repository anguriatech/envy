# envy auto

Opt this project into transparent shim injection (no more `envy run --` prefix).

## What it does

`envy auto on` opts the current project in (`auto_inject = true` in
`envy.toml`). Combined with `envy reshim` and shims on `PATH`, bare
`npm run dev` just works — secrets stay scoped to the child process and the
parent shell stays clean. `envy auto off` returns shims to transparent
pass-through.

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
| `on` | Enable shim injection for this project (then `envy reshim`) |
| `off` | Disable shim injection for this project |
| `status` | Show whether shim injection is on or off (default when omitted) |

## Examples

```bash
# Opt in, then generate shims
envy auto on
#   ✓ auto-inject enabled for my-project.
#   next steps:
#     1. envy reshim
#     2. envy shell-init zsh >> ~/.zshrc

# Check state
envy auto status
#   auto-inject: on (my-project)

# Opt out again (shims pass through without secrets)
envy auto off
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

`auto` only flips the `auto_inject` flag in `envy.toml` (text edit — your
`rotation_reminder_days` and comments are preserved), then prints the two
setup steps. The actual injection is done by shims ([envy reshim](envy-reshim.md)
generates them, [envy exec](envy-exec.md) runs them): environment selection is
`$ENVY_ENV` or `development`, and `ENVY_AUTO_INJECT=0` disables injection
globally.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Manifest not found (run `envy init` first) |
| `4` | Vault failure |

## Related commands

- [envy reshim](envy-reshim.md) — generate shims after opting in
- [envy shell-init](envy-shell-init.md) — the one-time `PATH` line shims need
- [envy doctor](envy-doctor.md) — verify the whole setup
- [envy run](envy-run.md) — scoped one-shot injection (equivalent guarantees)
- [envy init](envy-init.md) — `envy init --auto-inject` opts in from the start
