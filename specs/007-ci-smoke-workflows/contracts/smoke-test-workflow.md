# Contract: smoke-test.yml — Release Smoke Test Workflow

## Trigger Contract

```
on:
  release:
    types: [published]
```

## Matrix Contract

| Key | Value |
|-----|-------|
| `strategy.fail-fast` | `false` |
| `matrix.os` | `[ubuntu-latest, macos-latest, windows-latest]` |
| `runs-on` | `${{ matrix.os }}` |

## Step Contract (ordered)

| Step | Platform | Shell | Description |
|------|----------|-------|-------------|
| Install envy (Unix) | Linux + macOS | bash | `curl \| sh` from release assets |
| Install envy (Windows) | Windows | powershell | `irm \| iex` from release assets |
| Add envy to PATH | all | default | Source shell profile or add to `$GITHUB_PATH` |
| `envy --version` | all | default | Confirm binary runs |
| Create temp project dir + `cd` | all | default | Isolated working directory |
| `envy init` | all | default | Assert `envy.toml` created |
| `envy set DB_PASS=secret123 -e production` | all | default | Store test secret |
| `envy encrypt -e production` | all | default | Seal vault; `ENVY_PASSPHRASE=top-secret` via step env |
| Assert `envy.enc` in current dir | all | default | File existence check (native per platform) |
| `envy rm DB_PASS -e production` | all | default | Clear local state |
| `envy decrypt` | all | default | Restore from artifact; `ENVY_PASSPHRASE=top-secret` via step env |
| `envy get DB_PASS -e production` → assert `secret123` | all | default | Validate round-trip |

## Environment Variable Contract

| Variable | Step | Value | Injection method |
|----------|------|-------|-----------------|
| `ENVY_PASSPHRASE` | encrypt step | `top-secret` | step-level `env:` block |
| `ENVY_PASSPHRASE` | decrypt step | `top-secret` | step-level `env:` block |

## Invariant Contract

The following invariant MUST be verified by the smoke test and its failure MUST cause a non-zero exit:

> After `envy encrypt` is run from directory `$WORKDIR`, the file `$WORKDIR/envy.enc` MUST exist.
> The file MUST NOT be at `$(dirname $WORKDIR)/envy.enc` (the parent directory).

This directly validates the `artifact_path` bug fix (removal of `.parent()` call in `src/cli/mod.rs`).

## Exit Code Contract

- Any step failing causes the job to fail immediately
- All three matrix jobs must succeed for the smoke test to pass
- A failing smoke test on any platform MUST block release promotion (manual gate)
