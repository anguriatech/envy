# Tasks: CI and Smoke Test Workflows

**Input**: Design documents from `/specs/007-ci-smoke-workflows/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, contracts/ ✓, quickstart.md ✓

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[US1]**: CI Workflow (FR-001 – FR-009)
- **[US2]**: Smoke Test Workflow (FR-010 – FR-017)

---

## Phase 1: Setup

**Purpose**: Create the workflow directory structure and shared skeleton.

- [x] T001 Verify `.github/workflows/` directory exists in the repo root (create if absent)
- [x] T002 [P] Create empty `.github/workflows/ci.yml` with a top-level comment header
- [x] T003 [P] Create empty `.github/workflows/smoke-test.yml` with a top-level comment header

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish shared design decisions that both workflows depend on — `fail-fast`, permissions, and the dbus/gnome-keyring session pattern. Must be decided before either workflow is filled in.

**⚠️ CRITICAL**: Both user stories depend on these design decisions being locked.

- [x] T004 Document the `dbus-run-session` + `gnome-keyring-daemon --unlock` command string in a code comment at the top of `ci.yml` (research.md Decision 1) — this pattern is reused in both cargo test and E2E steps on Linux
- [x] T005 Confirm `shogo82148/actions-setup-perl@v1` is the correct action name by checking the GitHub Marketplace; note the pinned version (`5.38`) in plan.md

**Checkpoint**: Design decisions locked — workflow implementation can begin.

---

## Phase 3: User Story 1 — CI Workflow (`ci.yml`) (Priority: P1) 🎯 MVP

**Goal**: A single `ci.yml` that runs quality gates and all tests on ubuntu-latest, macos-latest, and windows-latest for every PR and push to master. (FR-001 – FR-009)

**Independent Test**: Open a test PR — all three matrix jobs must appear in the GitHub checks UI and pass. A deliberate `cargo fmt` violation must fail exactly the Linux job without cancelling macOS/Windows.

### 3.1 Trigger and Top-Level Configuration

- [x] T006 [US1] Add `on: pull_request / push` triggers targeting `master` in `.github/workflows/ci.yml` (FR-001)
- [x] T007 [US1] Add `permissions: contents: read` at workflow level in `.github/workflows/ci.yml`
- [x] T008 [US1] Add the matrix job `build` with `runs-on: ${{ matrix.os }}`, `strategy.fail-fast: false`, and `matrix.os: [ubuntu-latest, macos-latest, windows-latest]` in `.github/workflows/ci.yml` (FR-002)

### 3.2 Checkout and Rust Toolchain

- [x] T009 [US1] Add `actions/checkout@v4` step to `.github/workflows/ci.yml`
- [x] T010 [US1] Add `dtolnay/rust-toolchain@stable` step with `components: rustfmt, clippy` to `.github/workflows/ci.yml`

### 3.3 Linux Keyring Dependencies (FR-003)

- [x] T011 [US1] Add a step in `.github/workflows/ci.yml` guarded by `if: runner.os == 'Linux'` that runs `sudo apt-get update -q && sudo apt-get install -y libsecret-1-dev dbus-x11 gnome-keyring` (FR-003)

### 3.4 Windows Perl Installation (FR-004)

- [x] T012 [US1] Add a step in `.github/workflows/ci.yml` guarded by `if: runner.os == 'Windows'` that uses `shogo82148/actions-setup-perl@v1` with `perl-version: '5.38'`; place this step **before** any cargo invocation (FR-004)

### 3.5 Quality Gate (FR-005)

- [x] T013 [US1] Add `cargo fmt --check` step to `.github/workflows/ci.yml` (FR-005)
- [x] T014 [US1] Add `cargo clippy -- -D warnings` step to `.github/workflows/ci.yml` (FR-005)
- [x] T015 [US1] Add `cargo install cargo-audit --locked` followed by `cargo audit` as a single step in `.github/workflows/ci.yml` (FR-005)

### 3.6 cargo test (FR-006, FR-008)

- [x] T016 [US1] Add a `cargo test` step for **Linux** in `.github/workflows/ci.yml`, wrapped inside `dbus-run-session -- bash -c "echo '' | gnome-keyring-daemon --unlock --daemonize && cargo test"`, guarded by `if: runner.os == 'Linux'` (FR-006, FR-008)
- [x] T017 [US1] Add a `cargo test` step for **macOS and Windows** in `.github/workflows/ci.yml`, guarded by `if: runner.os != 'Linux'`, direct invocation with no dbus wrapper (FR-006)

### 3.7 E2E Script (FR-007, FR-008, FR-009)

- [x] T018 [US1] Add `cargo build` step before the E2E steps in `.github/workflows/ci.yml` to ensure the debug binary exists at `target/debug/envy` for the script to consume
- [x] T019 [US1] Add an E2E step for **Linux** in `.github/workflows/ci.yml`, wrapped with the same `dbus-run-session` pattern, running `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh`, guarded by `if: runner.os == 'Linux'` (FR-007, FR-008)
- [x] T020 [US1] Add an E2E step for **macOS and Windows** in `.github/workflows/ci.yml`, with `shell: bash` and `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh`, guarded by `if: runner.os != 'Linux'` (FR-007, FR-009)

**Checkpoint**: `ci.yml` is complete. Open a test PR to validate all three matrix jobs run and pass.

---

## Phase 4: User Story 2 — Smoke Test Workflow (`smoke-test.yml`) (Priority: P2)

**Goal**: A `smoke-test.yml` that fires after a release is published, installs the pre-compiled binary via official installers (no Rust toolchain), and runs the full round-trip test including the `envy.enc` placement invariant assertion. (FR-010 – FR-017)

**Independent Test**: Trigger manually against an existing release — all three matrix jobs must install `envy`, run `init → set → encrypt → rm → decrypt → get`, and assert `envy.enc` is in the working directory (not the parent).

### 4.1 Trigger and Top-Level Configuration

- [x] T021 [US2] Add `on: release: types: [published]` trigger to `.github/workflows/smoke-test.yml` (FR-010); note: fires after all release assets are uploaded, avoiding race with installer scripts
- [x] T022 [US2] Add `permissions: contents: read` at workflow level in `.github/workflows/smoke-test.yml`
- [x] T023 [US2] Add the matrix job `smoke` with `runs-on: ${{ matrix.os }}`, `strategy.fail-fast: false`, and `matrix.os: [ubuntu-latest, macos-latest, windows-latest]` in `.github/workflows/smoke-test.yml` (FR-011)

### 4.2 Checkout

- [x] T024 [US2] Add `actions/checkout@v4` step to `.github/workflows/smoke-test.yml` (needed to access any helper scripts if required; also ensures clean working directory)

### 4.3 Linux Keyring Setup for Smoke Test

- [x] T025 [US2] Add a step in `.github/workflows/smoke-test.yml` guarded by `if: runner.os == 'Linux'` that installs `libsecret-1-dev dbus-x11 gnome-keyring` (the binary uses the OS keyring for vault master key even in smoke test)

### 4.4 Install Envy via Official Installers (FR-012, FR-013)

- [x] T026 [US2] Add install step for **Linux and macOS** in `.github/workflows/smoke-test.yml` guarded by `if: runner.os != 'Windows'`: runs the `curl --proto '=https' --tlsv1.2 -LsSf .../envy-installer.sh | sh` one-liner and appends `$HOME/.cargo/bin` to `$GITHUB_PATH` (FR-012)
- [x] T027 [US2] Add install step for **Windows** in `.github/workflows/smoke-test.yml` guarded by `if: runner.os == 'Windows'`, `shell: powershell`: runs `irm .../envy-installer.ps1 | iex` and appends `$env:USERPROFILE\.cargo\bin` to `$env:GITHUB_PATH` (FR-012, FR-016)

### 4.5 Verify Binary

- [x] T028 [US2] Add a `run: envy --version` step (no `if` guard — all platforms) in `.github/workflows/smoke-test.yml` to confirm the binary is on PATH and executable before the round-trip begins

### 4.6 Round-Trip Test — Unix (FR-013, FR-014, FR-015, FR-017)

- [x] T029 [US2] Add the Unix round-trip step in `.github/workflows/smoke-test.yml` for **Linux**, wrapped with `dbus-run-session`, guarded by `if: runner.os == 'Linux'`, with `ENVY_PASSPHRASE: top-secret` in the step-level `env:` block, running the sequence: `mktemp -d` → `cd` → `envy init` → `envy set DB_PASS=secret123 -e production` → `envy encrypt -e production` → `test -f ./envy.enc || exit 1` → `envy rm DB_PASS -e production` → `envy decrypt` → assert `$(envy get DB_PASS -e production) = secret123` (FR-013, FR-014, FR-015, FR-017)
- [x] T030 [US2] Add the Unix round-trip step for **macOS** in `.github/workflows/smoke-test.yml`, guarded by `if: runner.os == 'macOS'`, same sequence as T029 but without `dbus-run-session` wrapper (FR-013, FR-014, FR-015, FR-017)

### 4.7 Round-Trip Test — Windows (FR-013, FR-014, FR-015, FR-016, FR-017)

- [x] T031 [US2] Add the Windows round-trip step in `.github/workflows/smoke-test.yml` guarded by `if: runner.os == 'Windows'`, `shell: powershell`, with `ENVY_PASSPHRASE: top-secret` in the step-level `env:` block, using `$ErrorActionPreference = 'Stop'`, running: `New-TemporaryFile`-based working dir → `envy init` → `envy set DB_PASS=secret123 -e production` → `envy encrypt -e production` → `if (-not (Test-Path ".\envy.enc")) { throw ... }` → `envy rm DB_PASS -e production` → `envy decrypt` → assert `(envy get DB_PASS -e production) -eq "secret123"` (FR-013, FR-014, FR-015, FR-016, FR-017)

**Checkpoint**: `smoke-test.yml` is complete. Trigger manually or wait for the next release.

---

## Phase 5: Validation and Polish

**Purpose**: Verify both workflows function end-to-end, confirm the `envy.enc` placement invariant, and clean up.

- [x] T032 [P] Add `workflow_dispatch:` trigger temporarily to `.github/workflows/smoke-test.yml` to enable manual runs without publishing a real release — remove after validation
- [x] T033 Trigger the CI workflow on a test branch; confirm all three matrix jobs run in parallel and all pass
- [x] T034 Introduce a deliberate `cargo fmt` violation, push to the test branch, confirm CI fails only on the formatting step (not mid-run) and that `fail-fast: false` keeps the other platform jobs running
- [x] T035 Manually trigger `smoke-test.yml` against the latest release; confirm each platform job installs `envy` without Rust, completes the round-trip, and the `envy.enc` assertion passes (not placed in parent directory)
- [x] T036 Remove the temporary `workflow_dispatch:` trigger from `.github/workflows/smoke-test.yml` added in T032
- [x] T037 [P] Update `CLAUDE.md` to add both new workflows to the project structure section

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately; T002 and T003 are parallel
- **Phase 2 (Foundational)**: Depends on Phase 1 — locks design decisions before YAML authoring begins
- **Phase 3 (US1 — ci.yml)**: Depends on Phase 2; tasks within are sequential per section
- **Phase 4 (US2 — smoke-test.yml)**: Depends on Phase 2; can proceed in parallel with Phase 3 since files are independent
- **Phase 5 (Validation)**: Depends on both Phase 3 and Phase 4 being complete

### User Story Dependencies

- **US1 (ci.yml)**: Independent — no dependency on US2
- **US2 (smoke-test.yml)**: Independent — no dependency on US1; both can be implemented simultaneously

### Within Each User Story

Tasks are sequential within each section (trigger → checkout → platform setup → quality → tests → E2E). Platform-specific steps within a section (e.g., T016 Linux / T017 macOS+Windows) are parallel by nature.

### Parallel Opportunities

- T002 and T003 (Phase 1): Create both empty files simultaneously
- T009 and T010 (Phase 3): Checkout and Rust toolchain steps can be authored in parallel
- T013, T014, T015 (Phase 3 quality gate steps): Can be authored simultaneously
- T016 and T017 (Phase 3 cargo test): Linux and non-Linux variants authored in parallel
- T019 and T020 (Phase 3 E2E): Linux and non-Linux variants authored in parallel
- Phase 3 and Phase 4 overall: Both workflows can be written simultaneously by different implementers

---

## Parallel Example: Phase 3 + Phase 4

```text
# Once Phase 2 is complete, both workflows can be authored simultaneously:

Implementer A — ci.yml (US1):
  T006 → T007 → T008 → T009+T010 → T011+T012 → T013+T014+T015 → T016+T017 → T018 → T019+T020

Implementer B — smoke-test.yml (US2):
  T021 → T022 → T023 → T024 → T025 → T026+T027 → T028 → T029+T030 → T031
```

---

## Implementation Strategy

### MVP (User Story 1 — ci.yml only)

1. Complete Phase 1 (T001–T003)
2. Complete Phase 2 (T004–T005)
3. Complete Phase 3 (T006–T020)
4. **STOP and VALIDATE**: Open a test PR — all three platform jobs must pass
5. Merge `ci.yml` — all future PRs are now gated automatically

### Full Delivery (+ Smoke Test)

6. Complete Phase 4 (T021–T031) — can overlap with Phase 3
7. Complete Phase 5 (T032–T037) — validate both workflows and clean up

---

## FR Traceability

| Requirement | Tasks |
|-------------|-------|
| FR-001 (CI trigger) | T006 |
| FR-002 (3-OS matrix, fail-fast) | T008 |
| FR-003 (Linux keyring deps) | T011 |
| FR-004 (Windows Perl) | T012 |
| FR-005 (quality gate: fmt, clippy, audit) | T013, T014, T015 |
| FR-006 (cargo test) | T016, T017 |
| FR-007 (E2E script, all platforms) | T019, T020 |
| FR-008 (Linux dbus-run-session) | T016, T019 |
| FR-009 (Windows shell: bash) | T020 |
| FR-010 (smoke trigger: release published) | T021 |
| FR-011 (smoke 3-OS matrix) | T023 |
| FR-012 (installer scripts only) | T026, T027 |
| FR-013 (envy.enc placement invariant) | T029, T030, T031 |
| FR-014 (full round-trip assertion) | T029, T030, T031 |
| FR-015 (exit non-zero on failure) | T029, T030, T031 |
| FR-016 (Windows ENVY_PASSPHRASE via env:) | T027, T031 |
| FR-017 (smoke exit non-zero on failure) | T029, T030, T031 |
