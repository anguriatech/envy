---

description: "Task list for Vault Core Data Model — 001-vault-db-schema"
---

# Tasks: Vault Core Data Model

**Input**: Design documents from `/specs/001-vault-db-schema/`
**Prerequisites**: plan.md ✅, spec.md ✅, data-model.md ✅, contracts/database-layer.md ✅

**Tests**: Included — all core database operations require unit tests per constitution
Principle III ("write unit tests for all core logic").

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing. All three user stories are P1; implementation order is US1 → US2 → US3
because each story builds on the previous layer's entities.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no shared dependencies at that point)
- **[Story]**: Which user story this task belongs to ([US1], [US2], [US3])
- Exact file paths are included in every description

---

## Phase 1: Setup

**Purpose**: Establish the Ubuntu system dependency, Cargo project, and 4-layer source
structure. No user story work can begin until this phase is complete.

> ⚠️ **Ubuntu system requirement**: The `keyring` crate requires `libsecret-1-dev` to
> compile on Linux. Run the following BEFORE `cargo build`:
> ```bash
> sudo apt-get install -y libsecret-1-dev pkg-config
> ```
> This is a one-time system setup step. CI/CD pipelines MUST install this package too.

- [x] T001 Verify `libsecret-1-dev` and `pkg-config` are installed on Ubuntu — run `dpkg -l libsecret-1-dev` and `pkg-config --version`; if missing, install with `sudo apt-get install -y libsecret-1-dev pkg-config`
- [x] T002 Create `Cargo.toml` at the repository root with binary crate metadata and all required dependencies — set `name = "envy"`, `edition = "2021"`, pin `rust-version` to current stable; add `[dependencies]`: `clap = { version = "4", features = ["derive"] }`, `rusqlite = { version = "0.31", features = ["bundled-sqlcipher"] }`, `uuid = { version = "1", features = ["v4"] }`, `keyring = "2"`, `thiserror = "1"`; add `[dev-dependencies]`: `tempfile = "3"`
- [x] T003 [P] Create the 4-layer source scaffold: `src/main.rs` (empty `fn main() {}`), `src/cli/mod.rs` (empty module), `src/core/mod.rs` (empty module), `src/crypto/mod.rs` (empty module), `src/db/mod.rs` (empty module) — add `mod cli; mod core; mod crypto; mod db;` to `src/main.rs`
- [x] T004 [P] Create the integration test directory scaffold: `tests/db/test_schema.rs`, `tests/db/test_projects.rs`, `tests/db/test_environments.rs`, `tests/db/test_secrets.rs`, `tests/db/test_security.rs` — each file starts with `// Integration tests for the db layer` comment only; no test functions yet
- [x] T005 Run `cargo build` after T002–T004 to verify the dependency graph compiles cleanly (no source logic yet — just scaffolding); fix any `Cargo.toml` errors before proceeding

**Checkpoint**: `cargo build` succeeds with zero errors. `libsecret-1-dev` confirmed installed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core database infrastructure that ALL three user stories depend on. No user
story implementation can begin until this phase is complete.

⚠️ **CRITICAL**: Do not start Phase 3 until ALL Phase 2 tasks pass `cargo test`.

- [x] T006 [P] Implement the `DbError` enum in `src/db/error.rs` using `thiserror` — derive `#[derive(Debug, thiserror::Error)]`; variants: `NotFound`, `AlreadyExists`, `ConstraintViolation(String)`, `IoError(String)`, `EncryptionError`, `MigrationError(String)`, `Internal(String)`; implement `#[error(...)]` display strings for each; add `pub use error::DbError;` to `src/db/mod.rs`
- [x] T007 [P] Implement newtype wrappers in `src/db/mod.rs`: `pub struct ProjectId(pub String)`, `pub struct EnvId(pub String)`, `pub struct SecretId(pub String)` — each must implement `Clone`, `Debug`, `PartialEq`; add a `fn as_str(&self) -> &str` method to each returning `&self.0`
- [x] T008 Implement the `Vault` struct and `Vault::open()` in `src/db/mod.rs` — `pub struct Vault { conn: rusqlite::Connection }`; `pub fn open(vault_path: &std::path::Path, master_key: &[u8]) -> Result<Vault, DbError>`: open or create the SQLCipher DB at `vault_path`, execute `PRAGMA key` with the hex-encoded master key, execute `PRAGMA foreign_keys = ON`, execute `PRAGMA journal_mode = WAL`, call `schema::run_migrations(&conn)` (stub for now), return `Ok(Vault { conn })`; map all `rusqlite::Error` to `DbError::Internal` via `map_err`; NO `.unwrap()` anywhere
- [x] T009 Implement `src/db/schema.rs` with the full migration runner — `pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), DbError>`: read `PRAGMA user_version`; if `0`, execute all three `CREATE TABLE IF NOT EXISTS` DDL statements from `data-model.md` (projects, environments, secrets) in order, then set `PRAGMA user_version = 1`; if `>= 1` do nothing; any SQL error maps to `DbError::MigrationError`; add `mod schema;` to `src/db/mod.rs`
- [x] T010 Implement `Vault::close()` in `src/db/mod.rs` — `pub fn close(self) -> Result<(), DbError>`: the `Connection` is consumed; execute `PRAGMA wal_checkpoint(TRUNCATE)` before drop; return `Ok(())`; map any error to `DbError::Internal`
- [x] T011 Write unit tests for schema and connection in `tests/db/test_schema.rs` — use `tempfile::NamedTempFile` for an isolated vault path; test: (a) `Vault::open` succeeds with a dummy 32-byte master key; (b) `PRAGMA user_version` returns `1` after open; (c) all three tables exist (query `sqlite_master` for each table name); (d) `Vault::open` on the same file a second time is idempotent (no error, `user_version` still `1`); (e) `PRAGMA foreign_keys` returns `1`; (f) `PRAGMA journal_mode` returns `"wal"`; NO `.unwrap()` — use `?` or `.expect("test setup")` with documented reason

**Checkpoint**: `cargo test` passes all schema tests. Foundation is ready — user story
implementation can begin.

---

## Phase 3: User Story 1 — Register a New Project in the Vault (Priority: P1) 🎯 MVP

**Goal**: The database layer can create, retrieve, list, and delete project records.
A project's UUID is stable and unique across any number of create calls.

**Independent Test**: Open a vault, call `create_project("my-app")`, call `get_project(id)`,
verify name matches. Call `create_project("my-app")` again — a new distinct UUID is returned
(projects are not deduplicated by name at the DB layer; that is the Core layer's concern).
Call `delete_project(id)`, verify `get_project(id)` returns `DbError::NotFound`.

### Implementation for User Story 1

- [x] T012 Define the `Project` struct in `src/db/projects.rs` — `pub struct Project { pub id: ProjectId, pub name: String, pub created_at: i64, pub updated_at: i64 }`; derive `Debug`, `Clone`; add `mod projects; pub use projects::Project;` to `src/db/mod.rs`
- [x] T013 [US1] Implement `Vault::create_project()` in `src/db/projects.rs` — signature: `pub fn create_project(&self, name: &str) -> Result<ProjectId, DbError>`; generate UUID v4 with `uuid::Uuid::new_v4().to_string()`; `INSERT INTO projects (id, name) VALUES (?1, ?2)`; return `Ok(ProjectId(uuid))`; map rusqlite errors to `DbError::Internal`; no `.unwrap()`
- [x] T014 [US1] Implement `Vault::get_project()` and `Vault::get_project_by_name()` in `src/db/projects.rs` — `get_project(id: &ProjectId)`: `SELECT id, name, created_at, updated_at FROM projects WHERE id = ?1`; map `QueryReturnedNoRows` → `DbError::NotFound`, other errors → `DbError::Internal`; `get_project_by_name(name: &str)`: same query with `WHERE name = ?1`
- [x] T015 [US1] Implement `Vault::list_projects()` in `src/db/projects.rs` — `SELECT id, name, created_at, updated_at FROM projects ORDER BY created_at ASC`; collect into `Vec<Project>`; empty result returns `Ok(vec![])`
- [x] T016 [US1] Implement `Vault::delete_project()` in `src/db/projects.rs` — `DELETE FROM projects WHERE id = ?1`; check `changes() == 0` → return `DbError::NotFound`; cascade to environments and secrets is automatic via FK `ON DELETE CASCADE`
- [x] T017 [US1] Write unit tests for project CRUD in `tests/db/test_projects.rs` — test: (a) `create_project` returns a valid UUID-formatted `ProjectId`; (b) `get_project` returns the correct name; (c) `get_project` on non-existent id returns `DbError::NotFound`; (d) `list_projects` returns projects in `created_at ASC` order; (e) `delete_project` succeeds and subsequent `get_project` returns `DbError::NotFound`; (f) `delete_project` on non-existent id returns `DbError::NotFound`; use `tempfile::NamedTempFile` for isolation; no `.unwrap()`

**Checkpoint**: `cargo test tests::db::test_projects` passes. User Story 1 is independently functional.

---

## Phase 4: User Story 2 — Segregate Secrets by Environment (Priority: P1)

**Goal**: The database layer can create environments scoped to a project, enforce case
normalization and uniqueness per project, and isolate secrets by environment boundary.

**Independent Test**: Create a project, create `development` and `production` environments
under it. Verify `list_environments` returns both. Verify inserting a duplicate `development`
returns `DbError::AlreadyExists`. Delete the project — verify both environments are gone
(cascade). Delete an environment — verify its secrets are also gone (cascade tested in US3).

### Implementation for User Story 2

- [x] T018 Define the `Environment` struct in `src/db/environments.rs` — `pub struct Environment { pub id: EnvId, pub project_id: ProjectId, pub name: String, pub created_at: i64, pub updated_at: i64 }`; derive `Debug`, `Clone`; add `mod environments; pub use environments::Environment;` to `src/db/mod.rs`
- [x] T019 [US2] Implement `Vault::create_environment()` in `src/db/environments.rs` — signature: `pub fn create_environment(&self, project_id: &ProjectId, name: &str) -> Result<EnvId, DbError>`; the `name` parameter MUST already be lowercased by the caller (document this in a `//` comment); generate UUID v4; `INSERT INTO environments (id, project_id, name) VALUES (?1, ?2, ?3)`; map `rusqlite::Error::SqliteFailure` with `SQLITE_CONSTRAINT_UNIQUE` (error code 2067) → `DbError::AlreadyExists`; other constraint errors → `DbError::ConstraintViolation`; other errors → `DbError::Internal`
- [x] T020 [US2] Implement `Vault::get_environment()` and `Vault::get_environment_by_name()` in `src/db/environments.rs` — `get_environment(id: &EnvId)`: `SELECT * FROM environments WHERE id = ?1`; map `QueryReturnedNoRows` → `DbError::NotFound`; `get_environment_by_name(project_id: &ProjectId, name: &str)`: `SELECT * FROM environments WHERE project_id = ?1 AND name = ?2`
- [x] T021 [US2] Implement `Vault::list_environments()` in `src/db/environments.rs` — `SELECT * FROM environments WHERE project_id = ?1 ORDER BY name ASC`; collect into `Vec<Environment>`
- [x] T022 [US2] Implement `Vault::delete_environment()` in `src/db/environments.rs` — `DELETE FROM environments WHERE id = ?1`; check `changes() == 0` → `DbError::NotFound`; cascade deletes secrets automatically
- [x] T023 [US2] Write unit tests for environment operations in `tests/db/test_environments.rs` — test: (a) `create_environment` succeeds with a lowercase name; (b) `create_environment` with a duplicate `(project_id, name)` returns `DbError::AlreadyExists`; (c) `get_environment_by_name` returns the correct record; (d) inserting an environment with a non-existent `project_id` returns `DbError::ConstraintViolation` (FK enforcement); (e) `delete_project` cascades to delete its environments (query `environments` table after project delete — expect empty); (f) environment name `CHECK(name = lower(name))` is enforced — attempt to insert uppercase name directly via raw SQL and verify constraint fails; no `.unwrap()`

**Checkpoint**: `cargo test tests::db::test_environments` passes. User Story 2 is independently functional.

---

## Phase 5: User Story 3 — Store and Retrieve Individual Secrets (Priority: P1)

**Goal**: The database layer stores and retrieves opaque ciphertext blobs with their
nonces, enforces one-value-per-key-per-environment via upsert, and never touches plaintext.
A direct binary inspection of the vault file MUST NOT reveal any stored value.

**Independent Test**: Create a project + environment. Call `upsert_secret` with fake
ciphertext bytes and a 12-byte nonce. Call `get_secret` — verify the exact same bytes are
returned. Call `upsert_secret` again with different ciphertext for the same key — verify
the new bytes are returned and `updated_at > created_at`. Read the vault `.db` file as raw
bytes — assert the original plaintext (if the test uses a detectable sentinel) is absent.

### Implementation for User Story 3

- [x] T024 Define the `SecretRecord` struct in `src/db/secrets.rs` — `pub struct SecretRecord { pub id: SecretId, pub environment_id: EnvId, pub key: String, pub value_encrypted: Vec<u8>, pub value_nonce: Vec<u8>, pub created_at: i64, pub updated_at: i64 }`; derive `Debug`, `Clone`; add `mod secrets; pub use secrets::SecretRecord;` to `src/db/mod.rs`
- [x] T025 [US3] Implement `Vault::upsert_secret()` in `src/db/secrets.rs` — signature: `pub fn upsert_secret(&self, env_id: &EnvId, key: &str, value_encrypted: &[u8], value_nonce: &[u8]) -> Result<SecretId, DbError>`; validate `value_nonce.len() == 12` and return `DbError::ConstraintViolation("nonce must be 12 bytes".into())` if not; generate UUID v4 for new id; use `INSERT OR REPLACE INTO secrets (id, environment_id, key, value_encrypted, value_nonce, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'), strftime('%s','now'))`; on replace (key already exists), the old `id` is lost — this is acceptable in Phase 1; map errors to `DbError` variants; the Database layer MUST NOT log or print `value_encrypted` or `value_nonce` anywhere
- [x] T026 [US3] Implement `Vault::get_secret()` in `src/db/secrets.rs` — `SELECT id, environment_id, key, value_encrypted, value_nonce, created_at, updated_at FROM secrets WHERE environment_id = ?1 AND key = ?2`; map `QueryReturnedNoRows` → `DbError::NotFound`; return full `SecretRecord`
- [x] T027 [US3] Implement `Vault::list_secrets()` in `src/db/secrets.rs` — `SELECT * FROM secrets WHERE environment_id = ?1 ORDER BY key ASC`; collect into `Vec<SecretRecord>`; empty result → `Ok(vec![])`
- [x] T028 [US3] Implement `Vault::delete_secret()` in `src/db/secrets.rs` — `DELETE FROM secrets WHERE environment_id = ?1 AND key = ?2`; check `changes() == 0` → `DbError::NotFound`
- [x] T029 [US3] Write unit tests for secret CRUD in `tests/db/test_secrets.rs` — test: (a) `upsert_secret` with valid 12-byte nonce and arbitrary ciphertext bytes succeeds; (b) `get_secret` returns the exact same `value_encrypted` and `value_nonce` bytes (assert byte-for-byte equality); (c) second `upsert_secret` for same key replaces the record — `get_secret` returns new ciphertext; (d) `upsert_secret` with nonce ≠ 12 bytes returns `DbError::ConstraintViolation`; (e) `delete_secret` succeeds; (f) `delete_secret` on non-existent key returns `DbError::NotFound`; (g) `delete_environment` cascades to delete all its secrets; no `.unwrap()`
- [x] T030 [US3] Write the defense-in-depth security test in `tests/db/test_security.rs` — (a) open a vault with a known 32-byte dummy key; (b) upsert a secret with `key = "SENTINEL_KEY"` and `value_encrypted = b"SENTINEL_PLAINTEXT_12345"` (simulating what the crypto layer would produce — this is ciphertext in real use, but for the test we use a known byte pattern); (c) close the vault; (d) read the entire vault `.db` file as `Vec<u8>` using `std::fs::read`; (e) assert that the byte sequence `b"SENTINEL_PLAINTEXT_12345"` does NOT appear anywhere in the file bytes (`assert!(!file_bytes.windows(sentinel.len()).any(|w| w == sentinel))`); this validates SQLCipher full-file encryption is active and the bundled-sqlcipher feature is correctly enabled

**Checkpoint**: `cargo test tests::db::test_secrets tests::db::test_security` passes.
User Story 3 is independently functional. All three stories are now complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates, compiler hygiene, and cross-story integration validation.

- [x] T031 [P] Run `cargo clippy -- -D warnings` and fix ALL warnings — zero clippy warnings MUST be present before merge; common issues to address: unused imports, missing `#[allow(dead_code)]` for intentionally empty stubs (cli/core/crypto modules), redundant clones
- [x] T032 [P] Run the full test suite with `cargo test` and confirm all tests pass — document test count in a PR comment; all five test files MUST have at least one passing test
- [x] T033 [P] Run `cargo audit` and verify zero known vulnerabilities in the dependency tree — output should show "0 vulnerabilities found"; if vulnerabilities are found, upgrade affected crates before proceeding
- [x] T034 Run the quickstart.md validation procedure from `specs/001-vault-db-schema/quickstart.md` — execute each numbered step in order; confirm: `PRAGMA cipher_version` returns a non-empty string, `PRAGMA user_version` returns `1`, security test passes, cascade delete test passes
- [x] T035 [P] Update `CLAUDE.md` if any technology was added beyond what is already listed — verify `rusqlite`, `uuid`, `keyring`, `clap`, `thiserror` are all listed under Active Technologies

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately after installing `libsecret-1-dev`
- **Foundational (Phase 2)**: Depends on Phase 1 completion — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Phase 2 completion — can start immediately after
- **User Story 2 (Phase 4)**: Depends on Phase 2; also depends on Project entities from US1
  (environments reference `project_id` — the `create_project` function must exist to write tests)
- **User Story 3 (Phase 5)**: Depends on Phase 2; also depends on `create_environment` from US2
  (secrets reference `environment_id`)
- **Polish (Phase 6)**: Depends on all user stories being complete

### Within Each Phase

- `DbError` (T006) and newtypes (T007) can be written in parallel — different files
- `Vault::open` (T008) depends on T006 and T007
- Schema migration (T009) depends on T008 (needs a connection)
- Within each user story: struct definition → CRUD implementation → unit tests
- Tests for each story MUST reference only entities created within that story's setup

### Parallel Opportunities

```bash
# Phase 1 (after T001, T002):
Task: T003 "Create 4-layer source scaffold"
Task: T004 "Create test directory scaffold"

# Phase 2:
Task: T006 "Implement DbError in src/db/error.rs"
Task: T007 "Implement newtype wrappers in src/db/mod.rs"
# Then sequentially: T008 → T009 → T010 → T011

# Phase 6:
Task: T031 "cargo clippy"
Task: T032 "cargo test"
Task: T033 "cargo audit"
Task: T035 "Update CLAUDE.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T005)
2. Complete Phase 2: Foundational (T006–T011)
3. Complete Phase 3: User Story 1 (T012–T017)
4. **STOP and VALIDATE**: `cargo test tests::db::test_schema tests::db::test_projects`
5. The database layer can register and retrieve projects — foundational MVP proven

### Incremental Delivery

1. Setup + Foundational → DB connection opens, schema migrates, tests pass
2. Add User Story 1 → Project CRUD tested → MVP anchor
3. Add User Story 2 → Environment isolation tested → `envy init` multi-env ready
4. Add User Story 3 → Secret store/retrieve + security test → full DB layer complete
5. Polish → clippy + audit + quickstart validation → ready for next feature branch

---

## Notes

- `[P]` tasks touch different files and have no shared in-progress dependencies
- All test files use `tempfile::NamedTempFile` — never write to `~/.envy/vault.db` in tests
- The dummy master key in tests can be any 32 bytes (e.g., `[0u8; 32]`) — key strength is irrelevant for schema tests but SQLCipher REQUIRES a non-empty key
- The DB layer NEVER encrypts or decrypts — it stores and returns `Vec<u8>` blobs verbatim
- `.unwrap()` is PROHIBITED; use `?` for propagation or `.expect("reason: ...")` in tests where the reason makes the panic logically impossible (document the reason inline)
- Run `cargo test -- --nocapture` to see `println!` output during test debugging
- Each user story phase produces an independently testable deliverable — stop at any checkpoint to validate before continuing
