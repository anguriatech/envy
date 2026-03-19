# Status: Feature 001 — Vault DB Schema

**Date completed**: 2026-03-18
**All 35 tasks**: `[x]` complete

---

## What was built

A fully encrypted, tested database layer for Envy. The vault is a SQLCipher-encrypted SQLite
file that stores projects, environments, and secrets in a strict 3-level hierarchy.

### Architecture position

```
[ cli/ ]    ←  stub (empty, ready for next feature)
[ core/ ]   ←  stub (empty, ready for next feature)
[ crypto/ ] ←  stub (empty, ready for next feature)
[ db/ ]     ←  ✅ COMPLETE — this feature
```

The DB layer is self-contained. The three layers above it are scaffolded but empty,
ready for the next features.

---

## Deliverables

### Source (`src/db/`)

| File | Responsibility |
|------|----------------|
| `error.rs` | `DbError` enum + `map_rusqlite_error`, `not_found_or` helpers |
| `schema.rs` | `CREATE TABLE` DDL for `projects`, `environments`, `secrets`; migration runner using `PRAGMA user_version` |
| `mod.rs` | `Vault` struct, `open`/`close`, `ProjectId`/`EnvId`/`SecretId` newtypes, diagnostic pragma helpers |
| `projects.rs` | `create_project`, `get_project`, `get_project_by_name`, `list_projects`, `delete_project` |
| `environments.rs` | `create_environment`, `get_environment`, `get_environment_by_name`, `list_environments`, `delete_environment` |
| `secrets.rs` | `upsert_secret`, `get_secret`, `list_secrets`, `delete_secret` |

### Tests — 38 passing, 0 failing

| File | Tests | What's covered |
|------|-------|----------------|
| `test_schema.rs` | 7 | Vault opens, user_version=1, all 3 tables exist, idempotent reopen, FK pragma, WAL mode, wrong key → `EncryptionError` |
| `test_projects.rs` | 9 | UUID format, CRUD, NotFound, list ordering, duplicate names |
| `test_environments.rs` | 11 | CRUD, AlreadyExists, FK violation, `CHECK(name=lower(name))`, cascade from project, cross-project isolation |
| `test_secrets.rs` | 9 | Upsert, byte-exact round-trip, overwrite, bad nonce, delete, cascade from environment, cross-env isolation |
| `test_security.rs` | 2 | Sentinel bytes absent from raw `.db` file (SQLCipher proof), `PRAGMA cipher_version` non-empty |

### Documentation

- `docs/developer-guide.md` — 9 sections: prerequisites, architecture, Rust patterns
  (including the `impl Vault` across files pattern), pragma ordering gotchas, test structure
- `CLAUDE.md` — updated with all 5 active crates and corrected commands

---

## Key technical invariants enforced

- **`PRAGMA key` always first** — `Vault::open` enforces the order; wrong-key error surfaces
  at `PRAGMA journal_mode = WAL` (the first real file read), not at `PRAGMA key`
- **`PRAGMA foreign_keys = ON`** on every connection — cascade deletes work as expected
- **No `.unwrap()` in `src/`** — all errors use `?` or `map_err`
- **`value_encrypted` / `value_nonce` never logged** — security contract documented and enforced
- **Environment names must be pre-lowercased by caller** — enforced in code + `CHECK(name = lower(name))` at DB level as second line of defense
- **Nonce validated as exactly 12 bytes** in application code before hitting the DB

---

## Quality gates

| Gate | Result |
|------|--------|
| `cargo test` | 38/38 ✓ |
| `cargo clippy -- -D warnings` | 0 warnings ✓ |
| `cargo audit` | 0 vulnerabilities ✓ |

> `cargo audit` reports 2 "unmaintained" warnings in transitive dependencies of `keyring`
> (`derivative`, `instant` via `zbus`). These are warnings, not vulnerabilities, and are not
> actionable until `keyring` releases an updated version.

---

## What's next

The vault layer is the foundation everything else builds on. Suggested next features:

1. **Crypto layer** — AES-256-GCM encrypt/decrypt + HKDF key derivation (`src/crypto/` stub is ready)
2. **Core layer** — business logic wiring crypto + db (`envy_set`, `envy_get`, key management)
3. **CLI layer** — `clap` commands (`envy add`, `envy get`, `envy run`, `envy list`, etc.)
