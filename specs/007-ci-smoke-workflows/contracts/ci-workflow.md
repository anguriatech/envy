# Contract: ci.yml — Cross-Platform CI Workflow

## Trigger Contract

```
on:
  pull_request:
    branches: [master]
  push:
    branches: [master]
```

## Matrix Contract

| Key | Value |
|-----|-------|
| `strategy.fail-fast` | `false` |
| `matrix.os` | `[ubuntu-latest, macos-latest, windows-latest]` |
| `runs-on` | `${{ matrix.os }}` |

## Step Contract (ordered)

| Step | Condition | Shell | Exit on failure |
|------|-----------|-------|-----------------|
| checkout | always | default | yes |
| Install Rust stable | always | default | yes |
| Install libsecret + dbus + gnome-keyring | `runner.os == 'Linux'` | bash | yes |
| Install Perl 5.38 | `runner.os == 'Windows'` | default | yes |
| `cargo fmt --check` | always | default | yes |
| `cargo clippy -- -D warnings` | always | default | yes |
| `cargo audit` | always | default | yes |
| `cargo test` | Linux: inside dbus-run-session; others: direct | default | yes |
| `tests/e2e_devops_scenarios.sh` | Linux: inside dbus-run-session; Windows: `shell: bash` | bash | yes |

## Environment Variable Contract

| Variable | Scope | Value | Notes |
|----------|-------|-------|-------|
| `ENVY_PASSPHRASE` | E2E script step env | (none — set by script internally) | Script uses its own internal passphrase values |

## Exit Code Contract

- Any step exiting non-zero causes the job to fail
- A job failure does NOT cancel sibling matrix jobs (`fail-fast: false`)
- All three matrix jobs must succeed for the overall CI check to pass
