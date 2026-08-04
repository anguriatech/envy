# Team sync — hand secrets to a teammate with `envy.enc`

The GitOps loop: **Dev A** seals secrets into `envy.enc` (safe to commit —
pure ciphertext, no key names, no values, no project id). **Dev B** pulls
the repo, runs `envy decrypt`, and gets the secrets — no Slack messages, no
shared spreadsheets.

```bash
cd examples/team-sync

# Dev A seals; the artifact is written to /tmp/handoff/ (the "git push")
ARTIFACT_OUT=/tmp/handoff ENVY_BIN=./target/debug/envy ./dev_a.sh

# Dev B receives it (the "git pull") on a fresh machine
ARTIFACT_IN=/tmp/handoff/envy.enc ENVY_BIN=./target/debug/envy ./dev_b.sh
```

## Prerequisites

- `envy` built or installed (see [basic](../basic/README.md)).
- Both scripts are headless: they export `ENVY_PASSPHRASE` (ephemeral
  keyring fallback) and `ENVY_PASSPHRASE_DEVELOPMENT` (the envelope
  passphrase). In reality the passphrase is shared out-of-band (password
  manager, meeting); here it is a hardcoded dummy value.

## Walkthrough

1. **Dev A** — `dev_a.sh`: `envy init`, `envy set` two dummy secrets, then
   `envy encrypt -e development` headless via `ENVY_PASSPHRASE_DEVELOPMENT`.
   This produces `envy.enc` in the project root and copies it to
   `ARTIFACT_OUT` when set.
2. **Dev B** — `dev_b.sh`: starts in a **fresh directory**, receives
   `envy.enc` via `ARTIFACT_IN`, runs `envy decrypt` headless, verifies the
   secret round-tripped, and runs a command with it injected.

Both scripts are idempotent: each run uses its own temp directory.

## What to try next

- `examples/ci-cd/` — the same loop, headless, in a GitHub Actions pipeline.
- `envy diff` / `envy status` — verify vault ↔ artifact sync before sealing.
