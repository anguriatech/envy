# envy shell-init

Print the one-time `PATH` line that activates shims.

## What it does

Prints `export PATH="$HOME/.envy/shims:$PATH"` in your shell's syntax. Paste
it **last** in your shell profile and restart: from then on, shimmed commands
(`npm`, `cargo`, …) resolve to `~/.envy/shims/` first and inject vault secrets
scoped to the child process — the parent shell stays clean. Do this once per
machine; then `envy auto on` + `envy reshim` per project.

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
# bash — preview, then append LAST to ~/.bashrc and restart
envy shell-init bash
envy shell-init bash >> ~/.bashrc

# zsh — append LAST to ~/.zshrc
envy shell-init zsh >> ~/.zshrc

# fish — append near the end of the fish config
envy shell-init fish >> ~/.config/fish/config.fish

# powershell — paste the output into $PROFILE
envy shell-init powershell

# nushell — paste the output into config.nu ($nu.config-path)
envy shell-init nushell

# direnv users — PATH_add ~/.envy/shims in the project's .envrc instead
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

The output is static text (no vault, manifest, or rc access — nothing is ever
installed automatically): one `PATH`-prepend line plus ordering guidance.
"Last in your rc" matters because version managers (fnm/nvm/mise/…) prepend on
every prompt and would otherwise shadow the shims. Verify with
`which -a npm` (shim first) and `envy doctor`.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (always — printing a line cannot fail) |

## Related commands

- [envy auto](envy-auto.md) — opt a project in (`envy auto on`)
- [envy reshim](envy-reshim.md) — generate the shims the `PATH` line activates
- [envy doctor](envy-doctor.md) — verify the line works
- [envy run](envy-run.md) — scoped one-shot injection without any setup
