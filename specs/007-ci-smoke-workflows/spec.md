# Feature Specification: CI and Smoke Test Workflows

**Feature Branch**: `007-ci-smoke-workflows`
**Created**: 2026-03-23
**Status**: Draft

## User Scenarios & Testing *(mandatory)*

### User Story 1 — CI Catches Breakage Before Merge (Priority: P1)

A contributor opens a pull request or pushes to master. The CI workflow runs automatically across all three supported platforms — Linux, macOS, and Windows — verifying that the code compiles, passes all quality checks, and all tests (unit, integration, and end-to-end) pass before the change can be merged.

**Why this priority**: Envy targets Linux, macOS, and Windows. A bug that only manifests on one platform must be caught before it ships. This is the primary safety net for every code change.

**Independent Test**: Can be fully tested by opening a PR with a deliberate compilation error on one platform and confirming the CI run fails on that platform while the others remain green.

**Acceptance Scenarios**:

1. **Given** a PR is opened against master, **When** the CI workflow triggers, **Then** it runs jobs for ubuntu-latest, macos-latest, and windows-latest in parallel.
2. **Given** `cargo fmt` detects unformatted code, **When** the quality step runs, **Then** the job fails and the PR cannot merge.
3. **Given** `cargo clippy` reports a warning, **When** the quality step runs with deny-warnings, **Then** the job fails.
4. **Given** `cargo audit` finds a known vulnerability in a dependency, **When** the audit step runs, **Then** the job fails.
5. **Given** a unit or integration test fails on Windows, **When** `cargo test` runs, **Then** the Windows job fails and the Linux/macOS jobs are unaffected.
6. **Given** the E2E bash script detects incorrect `envy.enc` placement, **When** `tests/e2e_devops_scenarios.sh` runs, **Then** the job fails and surfaces the scenario that broke.
7. **Given** all quality checks and tests pass on all three platforms, **When** the CI run completes, **Then** all matrix jobs report success.

---

### User Story 2 — Smoke Test Guards the Published Release Binary (Priority: P2)

After a new release is published on GitHub, the smoke test workflow installs the pre-compiled binary on a clean machine (no Rust toolchain) using the official installer scripts. It then exercises the full encrypt-decrypt round-trip to confirm the shipped artifact is functional on each supported platform.

**Why this priority**: `cargo test` runs against source code, not the published binary. A misconfigured release pipeline (e.g., wrong OpenSSL linkage, broken installer) would produce a binary that passes CI but is broken for users. This workflow catches exactly that class of defect.

**Independent Test**: Can be fully tested by triggering the workflow against an existing published release and confirming the binary installs, runs `envy init`, encrypts, and decrypts correctly on each platform.

**Acceptance Scenarios**:

1. **Given** a GitHub Release is published, **When** the smoke test workflow triggers, **Then** it runs on ubuntu-latest, macos-latest, and windows-latest.
2. **Given** a clean runner with no Rust toolchain, **When** the installer script runs, **Then** `envy` is available in PATH without manual PATH manipulation.
3. **Given** `envy init` is run in a temporary directory, **When** the command completes, **Then** `envy.toml` exists in that same directory.
4. **Given** a secret is stored with `envy set DB_PASS=secret123 -e production`, **When** `ENVY_PASSPHRASE=top-secret envy encrypt -e production` runs, **Then** `envy.enc` is created in the **same directory as `envy.toml`** (not the parent directory).
5. **Given** `envy rm DB_PASS -e production` clears the local secret and `envy decrypt` runs with the correct passphrase, **When** `envy get DB_PASS -e production` is called, **Then** it prints exactly `secret123`.
6. **Given** the smoke test passes on all three platforms, **When** the workflow completes, **Then** all jobs report success and the release is confirmed deployable.

---

### Edge Cases

- What happens when the Linux keyring daemon is not available in a headless CI environment? The workflow must start a mock dbus/gnome-keyring session before any test that touches the OS credential store.
- What happens when `ENVY_PASSPHRASE` is set to whitespace in a CI step? The binary must return a non-zero exit code immediately rather than hanging on interactive input (regression guard for the whitespace passphrase bug).
- What happens when the E2E bash script is executed on a Windows runner where the default shell is PowerShell? The step must explicitly declare `shell: bash` (Git Bash) to interpret the script correctly.
- What happens when Perl is absent on the Windows runner during `cargo build`? Vendored OpenSSL compilation requires Perl; the CI must install it before the build step, or the build fails with an opaque error.
- What happens if the smoke test runner already has Rust installed? The workflow must not assume Rust is absent — it must validate `envy` works as a standalone binary regardless of the runner's toolchain state.

## Requirements *(mandatory)*

### Functional Requirements

**CI Workflow (ci.yml)**

- **FR-001**: The CI workflow MUST trigger on every pull request targeting master and every direct push to master.
- **FR-002**: The CI workflow MUST run jobs in a matrix across ubuntu-latest, macos-latest, and windows-latest with `fail-fast: false`.
- **FR-003**: On Linux runners, the workflow MUST install `libsecret-1-dev`, `dbus-x11`, and `gnome-keyring` before any build or test steps.
- **FR-004**: On Windows runners, the workflow MUST install Perl 5.38 via `shogo82148/actions-setup-perl@v1` before any build step, to support vendored OpenSSL compilation.
- **FR-005**: The quality gate MUST run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo audit` in that order; any failure MUST abort the job.
- **FR-006**: The test gate MUST run `cargo test` to completion before executing the E2E script.
- **FR-007**: The E2E script (`tests/e2e_devops_scenarios.sh`) MUST be executed as part of CI on all three platforms.
- **FR-008**: On Linux, the E2E script MUST be invoked inside a `dbus-run-session` to provide a functional OS credential store in a headless environment.
- **FR-009**: On Windows, the E2E script step MUST specify `shell: bash` so it is interpreted by Git Bash rather than PowerShell.

**Smoke Test Workflow (smoke-test.yml)**

- **FR-010**: The smoke test workflow MUST trigger automatically when a GitHub Release is published.
- **FR-011**: The smoke test MUST run on ubuntu-latest, macos-latest, and windows-latest.
- **FR-012**: The smoke test MUST install the Envy binary using only the official installer scripts — `curl | sh` for Unix, `irm | iex` for Windows — and MUST NOT use `cargo install` or any Rust toolchain.
- **FR-013**: The smoke test MUST verify that `envy.enc` is created in the same directory as `envy.toml`, not in a parent directory, confirming the `artifact_path` bug fix is present in the release.
- **FR-014**: The smoke test MUST exercise the full round-trip: `init` → `set` → `encrypt` → `rm` → `decrypt` → `get`, and assert the final `get` output matches the original value exactly.
- **FR-015**: On Windows, the `ENVY_PASSPHRASE` environment variable MUST be injected using a PowerShell-compatible mechanism (e.g., step-level `env:` block).
- **FR-016**: The smoke test MUST exit non-zero if any step in the round-trip fails, causing the workflow job to fail.

### Key Entities

- **CI Workflow**: A GitHub Actions workflow (`ci.yml`) that enforces code quality and correctness on every code change across all supported platforms.
- **Smoke Test Workflow**: A GitHub Actions workflow (`smoke-test.yml`) that validates the published release binary on clean machines using only public installer scripts.
- **Matrix Job**: A workflow job that runs the same steps on multiple runner images in parallel, with independent pass/fail status per platform.
- **OS Keyring Mock**: A `dbus-run-session` + `gnome-keyring` process started on Linux headless runners to provide a functional OS credential store without a desktop session.
- **Round-Trip Test**: The sequence `init → set → encrypt → rm → decrypt → get` that validates the full lifecycle of a secret through Envy's encryption layer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A code change that breaks compilation on any single supported platform is caught before it reaches master — 100% of the time.
- **SC-002**: A formatting or linting violation is caught on the first CI run after it is introduced, with no false negatives.
- **SC-003**: The E2E scenario script completes without human intervention on all three platforms in CI; no passphrase prompts or manual confirmations occur.
- **SC-004**: After a release is published, the smoke test completes on all three platforms within 10 minutes of the release event.
- **SC-005**: The smoke test confirms that `envy.enc` is placed alongside `envy.toml` (not in the parent directory) for 100% of smoke test runs, validating the `artifact_path` fix is present in every release.
- **SC-006**: A developer can identify the root cause of a CI failure — which platform, which step, and which test — from the workflow log output alone, without downloading additional artifacts.

## Assumptions

- The repository already contains `tests/e2e_devops_scenarios.sh` and it is executable (`chmod +x`).
- The `bundled-sqlcipher-vendored-openssl` feature is already configured in `Cargo.toml`; Perl is the only additional system dependency needed on Windows for vendored OpenSSL compilation.
- Git Bash is available on GitHub's `windows-latest` runner (it is, by default as part of Git for Windows).
- The `shogo82148/actions-setup-perl@v1` action is publicly available and suitable for this use.
- The smoke test scope is limited to the single-user round-trip; Progressive Disclosure and multi-environment scenarios are covered by the E2E script in CI and are out of scope for the smoke test.
- `ENVY_PASSPHRASE` is used throughout both workflows; no interactive passphrase prompt is acceptable in any CI or smoke test step.
- On Linux CI, the `dbus-run-session` + `gnome-keyring` pattern is sufficient to satisfy the `keyring` crate's Secret Service dependency without requiring a full desktop environment.
