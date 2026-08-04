# CI/CD — headless envy in pipelines

Secrets live in the repo's encrypted artifact (`envy.enc`) and in vault
actions' secrets as the envelope passphrase. Pipelines decrypt headlessly,
**gate** on sync state with `envy status --format json`, and run with
injected secrets. Nothing is ever a plaintext `.env` in the repo.

## Running the headless loop

```bash
cd examples/ci-cd
ENVY_BIN=./target/debug/envy ./headless.sh
```

`headless.sh` walks the exact loop a pipeline would:

1. `envy init` + `envy set` — provision (in CI, secrets come from the vault
   action; here, dummy values).
2. `ENVY_PASSPHRASE_DEVELOPMENT=... envy encrypt -e development` — seal
   headlessly (no prompts; stdin from `/dev/null`).
3. `envy status --format json` — assert the environment is `in_sync` via
   `jq` (the quality gate from [CI/CD Integration](../../README.md)).
4. `envy diff` — exit `0` when vault == artifact, `1` when they drift, so
   pipelines can gate without masking real errors (exit `2+` is an error).
5. `envy run -- cmd` — inject secrets into the pipeline step.

## Copyable GitHub Actions workflow

`workflow.yml` is a complete job: decrypt the committed artifact with a
repo secret (`ENVY_KEY`), gate on sync state, then run a step with secrets
injected. Add it to `.github/workflows/` and set the `ENVY_KEY` secret.

## Why `ENVY_PASSPHRASE` and `ENVY_PASSPHRASE_DEVELOPMENT`

- `ENVY_PASSPHRASE` — activates envy's keyring fallback (deterministic
  ephemeral key), so the local vault works without a keyring daemon. CI also
  sets `CI`, which does the same. Required for `init`/`set`/`get`/`run`.
- `ENVY_PASSPHRASE_DEVELOPMENT` — the envelope passphrase sealing
  `envy.enc`; `encrypt`/`decrypt` use it. Name per environment:
  `ENVY_PASSPHRASE_STAGING`, `ENVY_PASSPHRASE_PRODUCTION`, ...

## Exit codes to gate on

| Code | Meaning |
|------|---------|
| `0` | Success / no drift / no leaks |
| `1` | `envy diff`: differences found; `envy scan`: leaks found |
| `2+` | Error (invalid input, crypto, vault) — fail fast |
