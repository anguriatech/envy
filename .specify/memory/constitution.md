<!--
SYNC IMPACT REPORT
==================
Version change: 1.0.0 → 1.1.0
Modified principles:
  - IV. Modularity: Expanded from 3 layers to 4 explicit layers; added Core/Business Logic
    layer; corrected dependency flow to match all four layers.
Modified sections:
  - Technology Stack: Mandated `rusqlite` with `bundled-sqlcipher` feature for AES-256
    encryption at rest; mandated `keyring` crate for OS Credential Manager integration.
Removed sections: N/A
Templates requiring updates:
  ✅ .specify/memory/constitution.md (this file — updated)
  ⚠ .specify/templates/plan-template.md — Constitution Check section uses generic gates;
      update to reference Security/Determinism/Modularity (4-layer) gates when generating
      the first plan.
  ⚠ .specify/templates/tasks-template.md — task scaffolding is language-agnostic;
      Rust-specific tasks (unit tests per module, no-unwrap audit, 4-layer separation,
      sqlcipher setup, keyring integration) must be added explicitly when running
      /speckit.tasks.
  ✅ .specify/templates/spec-template.md — no structural changes required; language-agnostic.
Follow-up TODOs:
  - TODO(RATIFICATION_DATE): Set to 2026-03-18 (date of initial constitution creation).
      Confirm if a different project start date should be used.
-->

# Envy Constitution

## Core Principles

### I. Security by Default

Secrets — passwords, private keys, API tokens, or any sensitive credential — MUST NEVER be
written to disk in plaintext form. All persistent storage of sensitive data MUST use
encryption at rest before any write operation occurs. In-memory representations of secrets
MUST be zeroed or dropped as early as possible after use. Logging MUST NOT emit secret
values, even partially or in hashed form unless explicitly designed for audit purposes.

**Rationale**: A single accidental plaintext write to disk, swap, or a log file can
permanently compromise user credentials. The cost of enforcing encryption uniformly is far
lower than the cost of a breach.

### II. Determinism

The CLI's behavior MUST be 100% predictable and reproducible given the same inputs and
environment. This means:

- All randomness MUST be seeded deterministically or sourced from a cryptographically secure
  RNG that is explicitly documented as such.
- No silent defaults that change based on environment, locale, or system state unless
  documented and testable.
- Command output format MUST NOT change across runs for the same input.
- Error messages MUST be stable and machine-parseable where applicable.

**Rationale**: Unpredictable behavior undermines user trust and makes automated pipelines
brittle. A CLI tool must be a reliable building block.

### III. Rust Best Practices

All Rust code MUST adhere to the following non-negotiable standards:

- Error propagation MUST use `Result<T, E>` with descriptive, typed errors. `anyhow` or a
  custom error enum is preferred over opaque strings.
- `.unwrap()` and `.expect()` are PROHIBITED except where the surrounding code makes it
  statically or logically impossible to panic — and in such cases, the reason MUST be
  documented in an inline comment immediately before the call.
- Unit tests MUST be written for all core logic (cryptography layer, database layer,
  business rules). Integration tests MUST cover primary CLI workflows.
- Code MUST compile with zero warnings under `cargo build` and pass `cargo clippy -- -D warnings`.
- Dependencies MUST be audited with `cargo audit` before release.

**Rationale**: Rust's type system is a safety net; circumventing it via panics trades
compile-time guarantees for runtime crashes. Consistent test coverage ensures regressions
are caught before they reach users.

### IV. Modularity

The codebase MUST maintain strict separation between four layers, each in its own Rust
module or crate:

- **UI/CLI layer** (`clap`): All argument parsing, user interaction, and output formatting.
  MUST NOT directly access the database, cryptographic primitives, or business logic
  internals — all operations MUST be delegated to the Core layer.
- **Core/Business Logic layer**: Orchestrates all application operations and enforces domain
  rules. MUST NOT import from the UI/CLI layer. Coordinates between the Cryptography and
  Database layers.
- **Cryptography layer**: All encryption, decryption, key derivation, and hashing. MUST NOT
  import from the UI/CLI or Core layers. MUST NOT know about database schemas.
- **Database layer**: All SQLite persistence logic (reads, writes, migrations, schema). MUST
  NOT import from the UI/CLI or Core layers. MUST NOT perform cryptographic operations
  directly.

Cross-layer dependencies MUST flow in one direction only:

```
UI/CLI → Core/Business Logic → Cryptography
                             → Database
```

No layer MUST import from a layer above it in this hierarchy.

**Rationale**: Tight coupling between layers makes security audits harder, testing more
fragile, and refactoring expensive. Four explicit layers — rather than an implicit
"business logic" — make the dependency contract unambiguous and each layer independently
verifiable.

### V. Language

All of the following MUST be written strictly in English:

- Source code identifiers (variables, functions, types, modules, constants)
- Inline and block comments
- Commit messages and PR descriptions
- Technical documentation (`docs/`, `README.md`, man pages, CLI help strings)
- Error messages surfaced to users

No other language is permitted in any project artifact.

**Rationale**: English is the lingua franca of open-source software. Consistent language
across all artifacts ensures any contributor can read, review, and maintain the entire
codebase without translation barriers.

## Technology Stack

This project is a Rust CLI tool. The following constraints apply to technology choices:

- **Language**: Rust (stable toolchain; MSRV to be documented in `Cargo.toml` via
  `rust-version`).
- **CLI Parsing**: `clap` (derive API preferred for declarative argument definitions).
- **Cryptography**: Use well-audited crates from the RustCrypto ecosystem
  (e.g., `aes-gcm`, `argon2`, `chacha20poly1305`). Custom cryptographic primitives are
  PROHIBITED.
- **Database**: `rusqlite` with the `bundled-sqlcipher` feature MUST be used. This provides
  transparent AES-256 encryption at rest for the entire SQLite database file. Plaintext
  SQLite (`rusqlite` without `bundled-sqlcipher`) is PROHIBITED. Remote databases are out
  of scope unless explicitly added via a constitution amendment.
- **Master Key Management**: The `keyring` crate MUST be used to store and retrieve the
  master encryption key exclusively via the OS Credential Manager (macOS Keychain, Windows
  Credential Manager, Linux Secret Service / libsecret). The master key MUST NEVER be
  stored in a config file, environment variable, or any other plaintext location on disk.
- **Testing**: `cargo test` for unit and integration tests. Test coverage for all
  cryptographic operations and database mutations is MANDATORY.
- **Auditing**: `cargo audit` MUST be run before any release tag is created.

## Development Workflow

All development MUST follow this process:

1. **Spec before code**: A feature specification MUST exist and be reviewed before
   implementation begins.
2. **Tests first**: Unit tests for new core logic MUST be written before or alongside
   implementation (not after). Tests MUST fail before implementation and pass after.
3. **Layer check**: Every PR MUST be reviewed for cross-layer dependency violations
   (Principle IV).
4. **Security gate**: Every PR touching the cryptography or database layer MUST include a
   reviewer comment confirming no plaintext secrets are written (Principle I).
5. **No-panic audit**: Every PR MUST confirm that any new `.unwrap()` or `.expect()` calls
   include the required inline justification comment (Principle III).
6. **English review**: All identifiers, comments, and documentation MUST be in English
   before merge (Principle V).

## Governance

This constitution supersedes all other project practices. Any practice that conflicts with
a principle in this document MUST be resolved in favor of the constitution.

**Amendment procedure**:
1. Open a PR with the proposed change to `.specify/memory/constitution.md`.
2. State the rationale, the version bump type (MAJOR/MINOR/PATCH), and list all artifacts
   that require updates.
3. At least one additional reviewer MUST approve before merge.
4. On merge, update `LAST_AMENDED_DATE` and `CONSTITUTION_VERSION`.

**Versioning policy**:
- MAJOR: Removal or backward-incompatible redefinition of an existing principle.
- MINOR: Addition of a new principle or materially expanded guidance.
- PATCH: Clarifications, wording improvements, typo fixes.

**Compliance review**: Each PR description MUST include a "Constitution Check" section
confirming compliance with all five core principles. If a violation is necessary, it MUST
be documented in a Complexity Tracking table with justification.

**Developer guidance**: For runtime development guidance — patterns, tooling, testing
conventions, and Rust-specific gotchas — see `docs/developer-guide.md`.

**Version**: 1.1.0 | **Ratified**: 2026-03-18 | **Last Amended**: 2026-03-18
