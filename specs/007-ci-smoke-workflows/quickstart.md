# Quickstart: CI and Smoke Test Workflows

## Scenario 1 — CI passes on a clean feature branch

```bash
# Developer pushes a branch and opens a PR
git push origin my-feature
gh pr create --title "Add new feature" --body "..."

# GitHub Actions triggers ci.yml automatically
# Three parallel jobs run: ubuntu-latest, macos-latest, windows-latest

# Each job runs in order:
# 1. cargo fmt --check          → fails if code is not formatted
# 2. cargo clippy -- -D warnings → fails on any warning
# 3. cargo audit                → fails on known CVE in dependencies
# 4. cargo test                 → runs all 57 unit + integration tests
# 5. tests/e2e_devops_scenarios.sh → runs 4 DevOps scenario tests (29 assertions)

# All three jobs succeed → PR is mergeable
```

## Scenario 2 — CI fails on Windows due to a platform-specific bug

```bash
# A change introduces a bug that only affects the Windows target
git push origin my-feature

# ci.yml runs all three matrix jobs
# ubuntu-latest  → PASS
# macos-latest   → PASS
# windows-latest → FAIL (cargo test fails)

# fail-fast: false ensures ubuntu and macos complete even though windows fails
# Developer can see: "3 jobs — 2 passed, 1 failed" in the PR checks
# No re-run needed to see the full picture
```

## Scenario 3 — Smoke test runs after a release is published

```bash
# Maintainer creates and publishes a release (via cargo-dist tag push)
git tag v0.2.0
git push origin v0.2.0

# release.yml (cargo-dist) runs and publishes GitHub Release with installers
# release: published event fires → smoke-test.yml triggers

# Ubuntu job:
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/anguriatech/envy/releases/latest/download/envy-installer.sh | sh
envy --version                                   # confirm binary works
mkdir /tmp/smoke && cd /tmp/smoke
envy init                                        # creates envy.toml
envy set DB_PASS=secret123 -e production
ENVY_PASSPHRASE=top-secret envy encrypt -e production
test -f ./envy.enc || exit 1                     # artifact_path bug guard
envy rm DB_PASS -e production
ENVY_PASSPHRASE=top-secret envy decrypt
RESULT=$(envy get DB_PASS -e production)
[ "$RESULT" = "secret123" ] || exit 1            # round-trip verified

# Same round-trip runs on macos-latest and windows-latest
# All three pass → release is confirmed deployable
```

## Scenario 4 — Smoke test catches a broken Windows binary

```bash
# A release is published with a Windows binary that has a missing DLL
# (e.g., libcrypto-3-x64.dll not statically linked — the bug from v0.1.0)

# smoke-test.yml triggers on release: published
# ubuntu-latest  → PASS  (binary works fine)
# macos-latest   → PASS  (binary works fine)
# windows-latest → FAIL  (envy --version produces no output, exits non-zero)

# Maintainer sees the failure, reverts the release, and fixes the build
# The Perl installation step in ci.yml is the preventive fix for this exact bug
```

## Scenario 5 — envy.enc placement assertion catches a regression

```bash
# A future change accidentally reintroduces the .parent() call in artifact_path()

# ci.yml → cargo test → unit tests catch it (artifact_path tests)
# ci.yml → e2e_devops_scenarios.sh → scenario 1 fails (envy.enc not found at expected path)

# If somehow the regression slips through to a release:
# smoke-test.yml → after envy encrypt, checks test -f ./envy.enc
# ./envy.enc does NOT exist (it's at ../envy.enc instead)
# The check exits 1 → smoke test fails → release blocked
```
