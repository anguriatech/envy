# Research: CI and Smoke Test Workflows

## Decision 1: Linux Headless Keyring Strategy

**Decision**: Use `dbus-run-session gnome-keyring-daemon --unlock` pattern to mock the OS Secret Service on Linux headless runners.

**Rationale**: The `keyring` crate on Linux uses the D-Bus Secret Service protocol (libsecret). GitHub Actions `ubuntu-latest` runners have no desktop session, so no keyring daemon is running by default. `dbus-run-session` starts a private D-Bus instance scoped to the command, and `gnome-keyring-daemon --unlock` starts the keyring daemon and unlocks its default collection in headless mode. This is the established community pattern for this exact problem.

**Alternatives considered**:
- `secret-tool` CLI mocking: Not viable — the `keyring` crate bypasses the CLI and speaks D-Bus directly.
- `kwallet` / KDE keyring: Available on Linux but not pre-installed on `ubuntu-latest`; gnome-keyring is.
- Skip keyring tests on Linux: Violates Constitution Principle III (mandatory test coverage); rejected.

**Implementation**: Wrap the `cargo test` and E2E script steps with `dbus-run-session -- bash -c "gnome-keyring-daemon --daemonize --unlock <<< '' && ..."`. The `<<< ''` heredoc supplies an empty password to unlock the default collection non-interactively.

---

## Decision 2: Windows Perl Installation

**Decision**: Use `shogo82148/actions-setup-perl@v1` at version `5.38` before any cargo build step on Windows.

**Rationale**: The `bundled-sqlcipher-vendored-openssl` feature in `rusqlite` vendors OpenSSL from source. OpenSSL's `Configure` build script is written in Perl. GitHub's `windows-latest` runner does not ship Perl by default (only Strawberry Perl as an optional tool, not always on PATH). The `shogo82148/actions-setup-perl` action installs a specific Perl version and ensures it is on PATH before Cargo starts.

**Alternatives considered**:
- `Chocolatey install strawberry-perl`: Slower, less reproducible versioning, no version pinning.
- Pre-built OpenSSL via `OPENSSL_DIR` env var: Requires the static `.lib` files at the exact expected path; fragile across runner image updates. The vendored approach is self-contained.
- Skip Windows build: Violates the cross-platform commitment; rejected.

---

## Decision 3: Smoke Test — Installer vs. cargo install

**Decision**: Use the official `envy-installer.sh` (Unix) and `envy-installer.ps1` (Windows) scripts from the release assets, not `cargo install`.

**Rationale**: The smoke test's purpose is to validate the *published binary*, not the source. Using `cargo install` would compile from source and miss the class of defects the smoke test is designed to catch (wrong OpenSSL linkage, incorrect archive structure, PATH setup by installer, etc.). The installer scripts are produced by `cargo-dist` and are the exact artifacts end users will run.

**Alternatives considered**:
- Download the `.tar.gz` / `.zip` archive directly and extract manually: More brittle; doesn't test the installer script itself, which is also a user-facing artifact.
- Use `gh release download` + manual PATH setup: Tests the binary in isolation but not the installer experience.

---

## Decision 4: Smoke Test Trigger

**Decision**: Trigger `smoke-test.yml` on the `release: published` event, not on tag push.

**Rationale**: The `release: published` event fires after the release is fully published (including all assets uploaded by `cargo-dist`). Triggering on tag push would race with the release workflow and the installer scripts would not yet exist as release assets.

**Alternatives considered**:
- `workflow_run` triggered by the release workflow: Adds coupling between workflow files; `release: published` is simpler and semantically correct.

---

## Decision 5: envy.enc Placement Assertion

**Decision**: After `envy encrypt`, use a platform-native file existence check (`test -f ./envy.enc` on Unix, `Test-Path ./envy.enc` on PowerShell) rather than running the E2E bash script.

**Rationale**: The smoke test does not have `jq` or bash guaranteed on Windows. A simple file existence check in the native shell is portable, readable, and directly asserts the invariant introduced by the `artifact_path` bug fix: `envy.enc` must appear in the current directory (alongside `envy.toml`), not one level up.

---

## Decision 6: Shell Handling on Windows

**Decision**: All Unix-compatible steps in `ci.yml` use the runner's default shell except the E2E script step, which explicitly declares `shell: bash`. The smoke test `smoke-test.yml` uses separate `if: runner.os != 'Windows'` / `if: runner.os == 'Windows'` steps with native shells.

**Rationale**: `tests/e2e_devops_scenarios.sh` is a bash script and must run in bash. On Windows, GitHub Actions defaults to PowerShell. Declaring `shell: bash` on that specific step uses Git Bash (bundled with the Windows runner). For the smoke test, the install/test steps differ fundamentally between PowerShell and bash, so conditional steps with native shells are cleaner than a single step with complex escape gymnastics.

---

## Decision 7: fail-fast Strategy

**Decision**: Set `fail-fast: false` on all matrix jobs in both workflows.

**Rationale**: If a platform-specific failure occurs, the other platforms must continue to completion so that the full failure surface is visible in one run. With `fail-fast: true` (the default), a Windows failure would cancel the macOS and Linux jobs, forcing a second re-run to observe those results.

---

## Decision 8: Secrets and Environment Variables

**Decision**: Use GitHub Actions step-level `env:` blocks (not `${{ secrets.X }}` inline in `run:` scripts) for `ENVY_PASSPHRASE`.

**Rationale**: Inlining secrets directly in `run:` command strings risks them appearing in log output. Step-level `env:` injection is the GitHub-recommended pattern — the value is masked in logs automatically. In the smoke test, `ENVY_PASSPHRASE` is a hardcoded test value (`top-secret`), not a real secret, but using the `env:` pattern is consistent and teaches correct usage.
