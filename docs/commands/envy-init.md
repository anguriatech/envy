# envy init

Initialise Envy in the current directory.

## What it does

Creates the `envy.toml` manifest (containing the project UUID) and registers a
new project in the encrypted vault (`~/.envy/vault.db`). Run this once per
project — before any other command. No secrets are written to disk.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy init [--auto-inject]
```

| Flag | Description |
|------|-------------|
| `--auto-inject` | Opt into transparent shim injection (`auto_inject = true` in `envy.toml`) |

## Examples

```bash
cd my-project
envy init
# or opt into shim injection from the start (no more `envy run --` prefix):
envy init --auto-inject
#   auto-inject enabled.
#   next steps:
#     1. envy reshim
#     2. envy shell-init zsh >> ~/.zshrc
```

## How it works

`envy init` generates a random `project_id` (UUID), stores it in `envy.toml`
(no secrets), and registers the project in the vault. The vault is created
on first use and encrypted with a master key stored in your OS credential
manager (Keychain / Credential Manager / Secret Service). In headless
environments without a keyring daemon, exporting `ENVY_PASSPHRASE` activates
the deterministic fallback key (ephemeral vault — dummy values only).

With `--auto-inject`, the manifest opts the project into shim injection from
the start. The command then prints the two setup steps (`envy reshim` for this
project's toolchains, shims on `PATH` once per machine) — nothing is installed
or generated automatically. Restart your shell after adding the `PATH` line.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `3` | `envy.toml` already exists (initialisation conflict) |
| `4` | Vault or keyring failure |

## Related commands

- [envy set](envy-set.md) — store your first secret after initialising
- [envy run](envy-run.md) — inject secrets into a child process
- [envy auto](envy-auto.md) — enable transparent shim injection later (`envy auto on`)
- [envy reshim](envy-reshim.md) — generate shims after opting in
- [envy shell-init](envy-shell-init.md) — one-time `PATH` setup shims need
