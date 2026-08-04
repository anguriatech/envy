# Envy Examples

Ready-to-run scenarios demonstrating real envy workflows. Every example is
verified in CI (E2E Scenario 15), so it never rots.

| Scenario | What it shows |
|----------|---------------|
| [basic/](basic/) | Local quickstart: init → set → list → run |
| [team-sync/](team-sync/) | Two-developer handoff via `envy.enc` (encrypt → decrypt) |
| [ci-cd/](ci-cd/) | Headless CI/CD usage: encrypt, status JSON gates, diff |
| [monorepo/](monorepo/) | Nested projects with independent secrets (feature 014) |

## Script contract

Every example script follows the same rules:

- **Bash** with `set -euo pipefail`.
- **Binary**: run with `ENVY_BIN=./target/debug/envy ./tutorial.sh`, or rely on
  the default `envy` on `PATH`.
- **Headless** (no prompts, no OS keyring daemon required): scripts export both
  env vars before running envy:
  - `ENVY_PASSPHRASE` — activates envy's keyring fallback, which uses a
    deterministic ephemeral key so the vault works without a keyring daemon.
    Required for `init`/`set`/`get`/`run`.
  - `ENVY_PASSPHRASE_DEVELOPMENT` — the envelope passphrase used by
    `encrypt`/`decrypt` to seal and unseal `envy.enc`.
  Because the fallback key is public, the vault is ephemeral: **only dummy
  values belong here — never real credentials.**
- **Idempotent**: safe to run twice (each run happens in its own temp dir).
- Prints a final success line (e.g. `basic: OK`) on completion.

## Running one example

```bash
cd basic
ENVY_BIN=./target/debug/envy ./tutorial.sh
```
