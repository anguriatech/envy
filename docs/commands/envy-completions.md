# envy completions

Generate shell completion scripts.

## What it does

Prints the tab-completion script for the given shell to stdout. Source the
output in your shell profile to enable command and flag completion for
`envy` — subcommands, flags, and environment values.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy completions SHELL
```

| Argument | Description |
|----------|-------------|
| `SHELL` | `bash`, `zsh`, or `fish` |

## Examples

```bash
# Bash
envy completions bash >> ~/.bash_completion

# Zsh
envy completions zsh > ~/.zfunc/_envy

# Fish
envy completions fish > ~/.config/fish/completions/envy.fish
```

## How it works

Generates the completion script from the clap command tree via
`clap_complete` and prints it to stdout — no vault or manifest access
required, so it works anywhere. The `completions` subcommand itself is
hidden from `envy --help` but fully functional.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |

## Related commands

- [envy run](envy-run.md) — what you'll type after tab-completion kicks in
- [envy --help](README.md) — full command overview
