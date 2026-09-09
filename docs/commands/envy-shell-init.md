# envy shell-init

Print the one-time shell setup snippet for transparent auto-injection.

## What it does

Prints shell code that installs a `cd`/prompt hook calling
[envy hook](envy-hook.md) on every navigation. Do this once per machine; then
`envy auto on` per project. After that, vault secrets are auto-exported when you
enter the project and unloaded when you leave — no `envy run --` prefix needed.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy shell-init [bash|zsh|fish|powershell|nushell]
```

| Argument | Description |
|----------|-------------|
| `[SHELL]` | Target shell (default: detected from `$SHELL`, else `bash`) |

## Examples

```bash
# bash — append to ~/.bashrc, then restart the shell
eval "$(envy shell-init bash)" >> /dev/null  # preview first!
envy shell-init bash >> ~/.bashrc

# zsh — append to ~/.zshrc
envy shell-init zsh >> ~/.zshrc

# fish — append to the fish config
envy shell-init fish >> ~/.config/fish/config.fish

# powershell — add to $PROFILE:
#   Invoke-Expression (& envy shell-init powershell | Out-String)
envy shell-init powershell

# nushell — paste into config.nu ($nu.config-path)
envy shell-init nushell

# direnv users — no shell-init needed; put this in the project's .envrc instead:
eval "$(envy hook --shell bash)"
# then: direnv allow
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

The snippet is static text (no vault or manifest access): it defines an
`__envy_hook` function that runs `envy hook --shell <shell>` and `eval`s its
stdout (warnings arrive on stderr, so they stay visible). Leaving a project
unsets previously exported keys via the `$__ENVY_KEYS` tracking variable.
`ENVY_AUTO_INJECT=0` disables the hook globally.

Security tradeoff: unlike the scoped `envy run`, auto-inject exports secrets into
your interactive shell, visible to every child process. Prefer `envy run` in CI
and for production deploys.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (always — printing a snippet cannot fail) |

## Related commands

- [envy auto](envy-auto.md) — opt a project in/out (`envy auto on`)
- [envy hook](envy-hook.md) — what the installed hook calls
- [envy run](envy-run.md) — scoped one-shot injection
