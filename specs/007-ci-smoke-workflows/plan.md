# Implementation Plan: CI and Smoke Test Workflows

**Branch**: `007-ci-smoke-workflows` | **Date**: 2026-03-23 | **Spec**: [spec.md](spec.md)

## Summary

Create two GitHub Actions workflow files:
1. **`ci.yml`** — runs on every PR and push to master; enforces code quality (fmt, clippy, audit) and executes all tests (`cargo test` + E2E bash script) across ubuntu-latest, macos-latest, and windows-latest; handles platform-specific setup (libsecret/dbus on Linux, Perl on Windows).
2. **`smoke-test.yml`** — triggers on release published; installs the pre-compiled binary via official installer scripts on clean machines (no Rust toolchain); exercises the full encrypt-decrypt round-trip and explicitly asserts that `envy.enc` is placed in the project root (not the parent directory), validating the `artifact_path` bug fix.

No Rust source changes. Both deliverables are pure YAML workflow files in `.github/workflows/`.

## Technical Context

**Language/Version**: GitHub Actions YAML; Rust 1.85 (stable) as toolchain installed in CI
**Primary Dependencies**: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `shogo82148/actions-setup-perl@v1`
**Storage**: N/A — workflows are stateless; no persistent state between jobs
**Testing**: `cargo test` (unit + integration), `tests/e2e_devops_scenarios.sh` (4 scenarios, 29 assertions)
**Target Platform**: GitHub Actions runners — ubuntu-latest, macos-latest, windows-latest
**Project Type**: CI/CD infrastructure (workflow configuration files)
**Performance Goals**: Full CI run completes in under 15 minutes; smoke test completes in under 10 minutes
**Constraints**: No interactive prompts anywhere; `ENVY_PASSPHRASE` must be injected via step-level `env:` block; Windows steps must use Git Bash for bash scripts

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | ✅ PASS | `ENVY_PASSPHRASE` injected via step-level `env:` (masked in logs); no secrets in `run:` strings |
| II. Determinism | ✅ PASS | `fail-fast: false` ensures full platform failure picture; pinned Perl version (5.38) |
| III. Rust Best Practices | ✅ PASS | CI enforces `cargo fmt`, `cargo clippy -- -D warnings`, `cargo audit`, `cargo test` |
| IV. Modularity | ✅ PASS | No source changes; workflows are infrastructure-only |
| V. Language | ✅ PASS | All YAML comments and step names are in English |

No violations. Complexity Tracking table not needed.

## Project Structure

### Documentation (this feature)

```text
specs/007-ci-smoke-workflows/
├── plan.md              ← this file
├── spec.md
├── research.md          ← Phase 0 complete
├── quickstart.md        ← Phase 1 complete
├── contracts/
│   ├── ci-workflow.md   ← Phase 1 complete
│   └── smoke-test-workflow.md  ← Phase 1 complete
├── checklists/
│   └── requirements.md
└── tasks.md             ← Phase 2 output (via /speckit.tasks — NOT created here)
```

### Source Code (repository root)

```text
.github/
└── workflows/
    ├── release.yml      ← existing (cargo-dist managed, allow-dirty: ["ci"])
    ├── ci.yml           ← NEW (this feature)
    └── smoke-test.yml   ← NEW (this feature)
```

**Structure Decision**: Additive only — two new YAML files in the existing `.github/workflows/` directory. No Rust source files are modified.

---

## Milestone 1: CI Workflow (`ci.yml`)

**Goal**: A single workflow file that enforces quality and runs all tests on every PR and push to master, handling all three platforms correctly.

### 1.1 Workflow Skeleton and Trigger

Create `.github/workflows/ci.yml` with:
- `on: pull_request` + `on: push` targeting `master`
- Top-level `permissions: contents: read` (read-only; no release creation)
- Matrix job `build` with `runs-on: ${{ matrix.os }}` and `fail-fast: false`
- `matrix.os: [ubuntu-latest, macos-latest, windows-latest]`

### 1.2 Rust Toolchain Setup

Install Rust stable using `dtolnay/rust-toolchain@stable` (preferred over `actions-rs` which is unmaintained). Components: `rustfmt`, `clippy`. No nightly toolchain needed.

Also install `cargo-audit` via `cargo install cargo-audit --locked` (or use a pre-built action if available).

### 1.3 Linux-Specific: Keyring Dependencies

Step with `if: runner.os == 'Linux'`:
```yaml
- name: Install keyring dependencies (Linux)
  if: runner.os == 'Linux'
  run: |
    sudo apt-get update -q
    sudo apt-get install -y libsecret-1-dev dbus-x11 gnome-keyring
```

This installs the Secret Service backend that `keyring` crate requires on Linux.

### 1.4 Windows-Specific: Perl for Vendored OpenSSL

Step with `if: runner.os == 'Windows'`:
```yaml
- name: Set up Perl (Windows — required to compile vendored OpenSSL)
  if: runner.os == 'Windows'
  uses: shogo82148/actions-setup-perl@v1
  with:
    perl-version: '5.38'
```

Must appear **before** any `cargo build` or `cargo test` step.

### 1.5 Quality Gate

Three sequential steps — any failure aborts the job:
1. `cargo fmt --check` — verify formatting
2. `cargo clippy -- -D warnings` — deny all warnings
3. `cargo install cargo-audit --locked && cargo audit` — vulnerability scan

### 1.6 cargo test

- **Linux**: Wrap inside `dbus-run-session` with gnome-keyring unlocked:
  ```bash
  dbus-run-session -- bash -c "
    echo '' | gnome-keyring-daemon --unlock --daemonize
    cargo test
  "
  ```
- **macOS**: Direct `cargo test` (Keychain available natively)
- **Windows**: Direct `cargo test` (Credential Manager available natively)

Use `if: runner.os == 'Linux'` / `if: runner.os != 'Linux'` conditional steps, or a single step with an inline shell conditional.

### 1.7 E2E Script

- **Linux**: Same `dbus-run-session` wrapper as cargo test
- **Windows**: `shell: bash` is MANDATORY; uses Git Bash
- **macOS**: Direct execution

```yaml
- name: Run E2E scenarios (Linux)
  if: runner.os == 'Linux'
  run: |
    dbus-run-session -- bash -c "
      echo '' | gnome-keyring-daemon --unlock --daemonize
      ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh
    "

- name: Run E2E scenarios (macOS / Windows)
  if: runner.os != 'Linux'
  shell: bash
  run: ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh
```

Note: `cargo test` runs in debug profile by default; build the debug binary before the E2E step, or run `cargo build` explicitly first.

---

## Milestone 2: Smoke Test Workflow (`smoke-test.yml`)

**Goal**: A workflow that fires after a release is published, installs the pre-compiled binary on clean machines (no Rust), and validates the full encrypt-decrypt round-trip including the `envy.enc` placement invariant.

### 2.1 Workflow Skeleton and Trigger

```yaml
on:
  release:
    types: [published]
```

Matrix: same three platforms with `fail-fast: false`. `permissions: contents: read`.

**No Rust toolchain step** — the smoke test must work without Rust installed.

### 2.2 Install Envy Binary

Two conditional steps — one for Unix, one for Windows:

**Unix (Linux + macOS)**:
```yaml
- name: Install envy (Unix)
  if: runner.os != 'Windows'
  run: |
    curl --proto '=https' --tlsv1.2 -LsSf \
      https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh
    echo "$HOME/.cargo/bin" >> $GITHUB_PATH
```

**Windows (PowerShell)**:
```yaml
- name: Install envy (Windows)
  if: runner.os == 'Windows'
  run: |
    irm https://github.com/anguriatech/envy/releases/latest/download/envy-installer.ps1 | iex
    "$env:USERPROFILE\.cargo\bin" | Out-File -FilePath $env:GITHUB_PATH -Append
  shell: powershell
```

### 2.3 Linux Keyring Setup for Smoke Test

Same `libsecret-1-dev`, `dbus-x11`, `gnome-keyring` install as in `ci.yml`. Required even in the smoke test because the binary uses the OS keyring for the vault master key.

### 2.4 Verify Binary

```yaml
- name: Verify envy binary
  run: envy --version
```

If this fails (e.g., DLL missing, PATH wrong), the job aborts immediately with a clear error.

### 2.5 Round-Trip Test — Unix

```yaml
- name: Smoke test round-trip (Unix)
  if: runner.os != 'Windows'
  env:
    ENVY_PASSPHRASE: top-secret
  run: |
    set -euo pipefail
    WORKDIR=$(mktemp -d)
    cd "$WORKDIR"

    envy init
    test -f envy.toml || (echo "ERROR: envy.toml not created" && exit 1)

    envy set DB_PASS=secret123 -e production
    envy encrypt -e production
    test -f ./envy.enc || (echo "ERROR: envy.enc not in project dir (artifact_path bug?)" && exit 1)

    envy rm DB_PASS -e production
    envy decrypt

    RESULT=$(envy get DB_PASS -e production)
    [ "$RESULT" = "secret123" ] || (echo "ERROR: got '$RESULT', expected 'secret123'" && exit 1)
    echo "Smoke test PASSED"
```

Note: `ENVY_PASSPHRASE` is injected via step-level `env:` so it applies to both `envy encrypt` and `envy decrypt` without appearing in the `run:` script.

**Linux wrapping**: This step must be wrapped with `dbus-run-session` on Linux, identical to the CI approach. Use `if: runner.os == 'Linux'` and `if: runner.os == 'macOS'` variants.

### 2.6 Round-Trip Test — Windows (PowerShell)

```yaml
- name: Smoke test round-trip (Windows)
  if: runner.os == 'Windows'
  shell: powershell
  env:
    ENVY_PASSPHRASE: top-secret
  run: |
    $ErrorActionPreference = 'Stop'
    $WORKDIR = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path "$($_.FullName)" }
    Set-Location $WORKDIR

    envy init
    if (-not (Test-Path "envy.toml")) { throw "ERROR: envy.toml not created" }

    envy set DB_PASS=secret123 -e production
    envy encrypt -e production
    if (-not (Test-Path ".\envy.enc")) { throw "ERROR: envy.enc not in project dir (artifact_path bug?)" }

    envy rm DB_PASS -e production
    envy decrypt

    $RESULT = envy get DB_PASS -e production
    if ($RESULT -ne "secret123") { throw "ERROR: got '$RESULT', expected 'secret123'" }
    Write-Host "Smoke test PASSED"
```

---

## Milestone 3: Validation

### 3.1 Manual Validation Checklist

Before merging, verify:
- [ ] `ci.yml` triggers on a test PR — all three matrix jobs run
- [ ] A deliberate fmt violation causes the ubuntu job to fail without cancelling macos/windows
- [ ] A deliberate clippy warning causes the affected job to fail
- [ ] `smoke-test.yml` is manually triggered (`workflow_dispatch` added temporarily OR triggered via a test release)
- [ ] On Linux smoke test: `envy.enc` exists in the temp working directory (not parent)
- [ ] On Windows smoke test: `envy get DB_PASS -e production` returns `secret123`

### 3.2 Self-Hosting

Once `ci.yml` is merged, all future PRs (including this one) are automatically gated by it. The workflow is self-validating from the first run.

---

## Key Design Decisions (from research.md)

| Topic | Decision |
|-------|----------|
| Linux keyring | `dbus-run-session` + `gnome-keyring-daemon --unlock` |
| Windows OpenSSL | `shogo82148/actions-setup-perl@v1` @ `5.38` before any cargo step |
| Smoke test install | Official installer scripts only — no `cargo install` |
| Smoke test trigger | `release: published` (not tag push — races with asset upload) |
| `envy.enc` assertion | Native file existence check (`test -f` / `Test-Path`) |
| Windows E2E script | `shell: bash` on the specific step |
| `ENVY_PASSPHRASE` | Step-level `env:` block (masked in logs) |
| fail-fast | `false` on all matrix jobs |
