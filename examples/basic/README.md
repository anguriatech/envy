# Basic quickstart — local secrets, zero friction

The hello-world of envy: store a secret, list it, and run a command with it
injected — no plaintext files, no shell history.

```bash
# From anywhere:
cd examples/basic
ENVY_BIN=./target/debug/envy ./tutorial.sh   # or just: ./tutorial.sh
```

## Prerequisites

- `envy` built: `cargo build` (from the repo root), or installed via the
  [official installers](https://github.com/anomalyco/envy/releases).
- The script is headless: it exports `ENVY_PASSPHRASE` (ephemeral keyring
  fallback) and `ENVY_PASSPHRASE_DEVELOPMENT` (envelope passphrase), so no
  keyring daemon or prompts are needed. **Only dummy values — never real
  credentials.**

## Walkthrough

1. `envy init` creates `envy.toml` (with a fresh `project_id`) and registers
   the project in the vault. `envy.toml` is safe to commit.
2. `envy set KEY=VALUE` stores two dummy secrets encrypted with AES-256-GCM.
3. `envy list` shows key names — values never printed by default.
4. `envy run -- echo "$DUMMY_KEY"` injects the secrets and runs the child —
   the value never touches disk.

The script is idempotent: every run uses its own temp directory.

## What to try next

- `examples/team-sync/` — hand secrets to a teammate via `envy.enc`.
- `docs/commands/` — the full per-command reference.
