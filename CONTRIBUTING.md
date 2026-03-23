# Contributing to Envy

Thank you for taking the time to contribute. Envy is a security-focused tool, and every improvement — whether a bug fix, a new feature, or better documentation — matters. This guide will get you from zero to a passing test suite as quickly as possible.

---

## Table of Contents

1. [Code of Conduct](#1-code-of-conduct)
2. [Setting Up Your Local Environment](#2-setting-up-your-local-environment)
3. [Architecture Overview](#3-architecture-overview)
4. [Running the Test Suite](#4-running-the-test-suite)
5. [Submitting a Pull Request](#5-submitting-a-pull-request)
6. [Reporting Bugs & Requesting Features](#6-reporting-bugs--requesting-features)

---

## 1. Code of Conduct

Be respectful and constructive. We welcome contributors of all experience levels. Security-related feedback should be reported privately — see [Reporting a Vulnerability](#reporting-a-vulnerability) below.

---

## 2. Setting Up Your Local Environment

### Prerequisites

You need **Rust stable (MSRV 1.85)** and a few system libraries depending on your OS.

#### Ubuntu / Debian

```bash
# OS keyring backend required by the `keyring` crate
sudo apt-get install -y libsecret-1-dev pkg-config

# Rust stable toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Optional: security vulnerability scanner
cargo install cargo-audit
```

#### macOS

`libsecret` is not needed — Envy uses the native macOS Keychain API. You do need the Xcode Command Line Tools for the C compiler that SQLCipher's bundled build requires:

```bash
xcode-select --install
```

#### Windows

The Rust `msvc` toolchain is required. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload, then install Rust via [rustup.rs](https://rustup.rs).

### Clone and build

```bash
git clone https://github.com/anguriatech/envy.git
cd envy
cargo build
```

A successful build confirms your environment is set up correctly.

### Verify

```bash
rustc --version    # rustc 1.85.x or later
cargo --version
```

---

## 3. Architecture Overview

Envy uses a **strict 4-layer architecture**. Understanding this rule is the single most important thing before writing any code.

```
┌──────────┐
│   cli/   │  parses arguments, formats output, manages exit codes
└────┬─────┘
     │ calls
┌────▼─────┐
│  core/   │  orchestrates operations, enforces business rules
└────┬─────┘
     │ calls
┌────▼─────┬──────────┐
│  crypto/ │   db/    │  independent leaf layers — neither knows about the other
└──────────┴──────────┘
```

**The key rule: CLI talks to Core, never directly to DB.**

| Layer | May import | Must NOT import |
|-------|-----------|-----------------|
| `cli` | `core` | `db`, `crypto` (two bootstrap exceptions allowed) |
| `core` | `db`, `crypto` | `cli` |
| `crypto` | — | everything else |
| `db` | — | everything else |

If you find yourself reaching for `db` from `cli`, add a function to `core` instead and call that. This keeps each layer independently testable and security-auditable.

The two permitted exceptions are `db::Vault::open` and `crypto::get_or_create_master_key`, which are infrastructure bootstrap operations that must happen before `core` can be called.

For a deeper explanation, see [docs/developer-guide.md](docs/developer-guide.md).

---

## 4. Running the Test Suite

All of these commands must pass before a PR will be merged.

### Format

```bash
cargo fmt --check
```

### Lint

```bash
cargo clippy -- -D warnings
```

Zero warnings are allowed. The `-D warnings` flag is enforced in CI.

### Unit and integration tests

```bash
cargo test
```

This runs all tests except those marked `#[ignore]` (which require a live OS keyring daemon). The test suite includes:

- `tests/db.rs` — database layer integration tests (in-memory SQLite)
- `tests/sync_artifact.rs` — end-to-end envy.enc seal/unseal pipeline

### Bash E2E scenarios

```bash
bash tests/e2e_devops_scenarios.sh
```

This script exercises the full CLI binary across nine real-world scenarios — single-env seal, progressive disclosure, headless CI passphrase, wrong passphrase graceful skip, and more. It requires a working OS keyring. Run it before submitting any PR that touches `cli/`, `core/sync.rs`, or `crypto/artifact.rs`.

### Security audit

```bash
cargo audit
```

Check for known vulnerabilities in the dependency tree. Flag any findings in your PR description.

---

## 5. Submitting a Pull Request

1. **Fork** the repository and create a branch from `master`.
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Write tests first.** Envy follows a TDD approach — write a failing test, then implement the code that makes it pass.

3. **Follow the layer architecture.** See Section 3 above.

4. **Run the full suite locally** before pushing:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   bash tests/e2e_devops_scenarios.sh
   ```

5. **Open a PR** against the `master` branch. Fill in the PR template — it exists to make review faster for everyone.

6. **Keep PRs focused.** One feature or fix per PR. If you spot unrelated issues, open a separate issue or PR.

### Commit message style

Follow conventional commits:

```
feat: add --json output flag to `envy list`
fix: handle empty passphrase in headless mode
docs: clarify progressive disclosure in README
test: add E2E scenario for legacy .env migration
```

---

## 6. Reporting Bugs & Requesting Features

Use the GitHub Issue templates:

- **[Bug report](.github/ISSUE_TEMPLATE/bug_report.md)** — for unexpected behaviour or crashes.
- **[Feature request](.github/ISSUE_TEMPLATE/feature_request.md)** — for new ideas or workflow improvements.

### Reporting a Vulnerability

**Do not open a public issue for security vulnerabilities.** Instead, use [GitHub's private vulnerability reporting](https://github.com/anguriatech/envy/security/advisories/new) or email the maintainers directly. We take security reports seriously and will respond promptly.