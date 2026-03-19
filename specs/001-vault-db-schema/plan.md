# Implementation Plan: Vault Core Data Model

**Branch**: `001-vault-db-schema` | **Date**: 2026-03-18 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-vault-db-schema/spec.md`

## Summary

Design and implement the foundational SQLite schema for the Envy encrypted vault
(`~/.envy/vault.db`). Three tables — `projects`, `environments`, `secrets` — organized in
a strict FK hierarchy with UUID primary keys, integer timestamps, cascade deletes, and
defense-in-depth per-secret AES-256-GCM encryption on top of full-file SQLCipher
encryption. The schema is versioned via `PRAGMA user_version` and migrates automatically
on first connection open.

## Technical Context

**Language/Version**: Rust stable (MSRV to be pinned in `Cargo.toml` `rust-version`)
**Primary Dependencies**: `rusqlite` (features: `bundled-sqlcipher`), `uuid` (features: `v4`), `keyring`
**Storage**: SQLite encrypted with SQLCipher (AES-256, full-file) at `~/.envy/vault.db`
**Testing**: `cargo test`, `tempfile` crate for isolated test vault instances
**Target Platform**: Linux, macOS, Windows (all supported by `rusqlite` + `keyring`)
**Project Type**: CLI tool (binary crate), 4-layer architecture per constitution
**Performance Goals**: Schema queries complete in <5ms on a standard developer laptop for up to 10k secrets
**Constraints**: Single statically linked binary; no runtime dependencies; offline-only in Phase 1
**Scale/Scope**: Phase 1 — single user, single machine; schema designed to extend to multi-user Phase 3

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Security by Default | Secret values MUST be stored encrypted (AES-256-GCM) even within the already-encrypted DB file (defense-in-depth). No plaintext bytes in any BLOB column. | ✅ PASS — `value_encrypted` + `value_nonce` columns; DB layer stores opaque bytes only. |
| II. Determinism | Schema creation MUST be idempotent (`CREATE TABLE IF NOT EXISTS`). Same inputs produce same schema state. | ✅ PASS — All DDL uses `IF NOT EXISTS`; `PRAGMA user_version` guards migrations. |
| III. Rust Best Practices | DB layer MUST return `Result<T, DbError>`. No `.unwrap()` in DB code. Unit tests for all operations. | ✅ PASS — Contract defines typed `DbError` enum; quickstart mandates test suite. |
| IV. Modularity | DB layer MUST NOT import from Core or CLI. Core calls DB operations; DB never calls Core. | ✅ PASS — Contract defines one-way interface: Core → Database only. |
| V. Language | All column names, comments, identifiers in English. | ✅ PASS — Schema uses English identifiers throughout. |

**Post-design re-check**: All gates still pass after Phase 1 design.

## Project Structure

### Documentation (this feature)

```text
specs/001-vault-db-schema/
├── plan.md              # This file
├── research.md          # Phase 0 output ✅
├── data-model.md        # Phase 1 output ✅
├── quickstart.md        # Phase 1 output ✅
├── contracts/
│   └── database-layer.md  # Phase 1 output ✅
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Entry point; CLI layer only
├── cli/                 # UI/CLI layer (clap)
│   └── mod.rs
├── core/                # Core/Business Logic layer
│   └── mod.rs
├── crypto/              # Cryptography layer
│   └── mod.rs
└── db/                  # Database layer (this feature's primary scope)
    ├── mod.rs           # Vault struct, open/close
    ├── schema.rs        # CREATE TABLE statements + migration runner
    ├── projects.rs      # Project CRUD operations
    ├── environments.rs  # Environment CRUD operations
    ├── secrets.rs       # Secret upsert/get/list/delete operations
    └── error.rs         # DbError enum

tests/
└── db/
    ├── test_schema.rs       # Schema creation, migration, idempotency
    ├── test_projects.rs     # Project CRUD
    ├── test_environments.rs # Environment CRUD + uniqueness
    ├── test_secrets.rs      # Secret upsert, overwrite, cascade
    └── test_security.rs     # Plaintext absence, nonce uniqueness
```

**Structure Decision**: Single-project Rust binary (Option 1). The 4-layer module
structure follows constitution Principle IV exactly. The `db/` module is this feature's
deliverable; other layers are scaffolded but empty until later features.

## Complexity Tracking

> No constitution violations require justification for this feature.
