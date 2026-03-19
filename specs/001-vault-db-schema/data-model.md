# Data Model: Vault Core Schema

**Feature**: 001-vault-db-schema
**Date**: 2026-03-18

---

## Overview

The Envy vault is a single SQLite database file at `~/.envy/vault.db`, encrypted at rest
via SQLCipher (AES-256, full-file). The schema consists of three tables organized in a
strict hierarchy:

```
projects
  └── environments (1:N, scoped to project)
        └── secrets (1:N, scoped to environment)
```

Future Phase 3 tables (`users`, `roles`, `audit_logs`) will attach to this hierarchy via
foreign keys on the existing UUID primary keys — no existing table will require alteration.

---

## Connection Initialization (Required Pragmas)

These MUST be executed in order at every connection open, before any other SQL:

```sql
-- 1. Provide the SQLCipher key (master key from OS Credential Manager via `keyring`)
PRAGMA key = '...';  -- injected at runtime, never hardcoded

-- 2. Enforce referential integrity (SQLite disables FK checks by default)
PRAGMA foreign_keys = ON;

-- 3. Enable Write-Ahead Logging for concurrent read access
PRAGMA journal_mode = WAL;
```

---

## Schema: CREATE TABLE Statements

### Table: `projects`

```sql
CREATE TABLE IF NOT EXISTS projects (
    -- Globally unique project identifier (UUID v4, hyphenated TEXT).
    -- Stable across machines; used as the FK anchor for environments
    -- and as the future anchor for users/roles in Phase 3.
    id          TEXT    NOT NULL PRIMARY KEY
                        CHECK(length(id) = 36),

    -- Human-readable project name (e.g., directory name or user-supplied label).
    -- Not required to be unique globally; uniqueness is enforced by the UUID.
    name        TEXT    NOT NULL
                        CHECK(length(name) > 0),

    -- Unix epoch (UTC, seconds). Set once on INSERT; never updated.
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- Unix epoch (UTC, seconds). Updated on every modification to this row.
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
```

---

### Table: `environments`

```sql
CREATE TABLE IF NOT EXISTS environments (
    -- Globally unique environment identifier (UUID v4, hyphenated TEXT).
    id          TEXT    NOT NULL PRIMARY KEY
                        CHECK(length(id) = 36),

    -- Parent project. CASCADE ensures no orphaned environments survive project deletion.
    project_id  TEXT    NOT NULL
                        REFERENCES projects(id) ON DELETE CASCADE,

    -- Environment label, normalized to lowercase before INSERT (e.g., 'development',
    -- 'staging', 'production'). The CHECK constraint enforces lowercase at the DB level
    -- as a second line of defense after application normalization.
    name        TEXT    NOT NULL
                        CHECK(name = lower(name))
                        CHECK(length(name) > 0),

    -- Unix epoch (UTC, seconds).
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- One environment name per project. Prevents accidental duplicate environments
    -- (e.g., two 'production' rows for the same project).
    UNIQUE(project_id, name)
);
```

---

### Table: `secrets`

```sql
CREATE TABLE IF NOT EXISTS secrets (
    -- Globally unique secret identifier (UUID v4, hyphenated TEXT).
    -- Serves as the stable FK target for future `audit_logs` in Phase 3.
    id                  TEXT    NOT NULL PRIMARY KEY
                                CHECK(length(id) = 36),

    -- Parent environment. CASCADE ensures no orphaned secrets survive
    -- environment deletion.
    environment_id      TEXT    NOT NULL
                                REFERENCES environments(id) ON DELETE CASCADE,

    -- Secret key name (e.g., 'DATABASE_URL', 'STRIPE_KEY').
    -- The CHECK at the DB level requires non-empty; format validation
    -- (uppercase, underscores) is the CLI layer's responsibility.
    key                 TEXT    NOT NULL
                                CHECK(length(key) > 0),

    -- Defense-in-depth: the secret value is encrypted with AES-256-GCM
    -- using a key derived from the master key + this row's UUID (HKDF-SHA256),
    -- even though the entire DB file is already AES-256 encrypted by SQLCipher.
    -- This column stores the raw ciphertext bytes.
    value_encrypted     BLOB    NOT NULL,

    -- The 12-byte (96-bit) random nonce used for this row's AES-256-GCM encryption.
    -- A unique nonce per row ensures identical plaintext values produce different
    -- ciphertexts, preventing pattern analysis.
    value_nonce         BLOB    NOT NULL
                                CHECK(length(value_nonce) = 12),

    -- Unix epoch (UTC, seconds). Set once on INSERT; never updated.
    created_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- Unix epoch (UTC, seconds). Refreshed on every UPDATE to this row.
    updated_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    -- One value per key per environment. An INSERT OR REPLACE (upsert) against
    -- this constraint implements the 'envy set' overwrite behavior atomically.
    UNIQUE(environment_id, key)
);
```

---

## Indexes

SQLite automatically creates a B-tree index on every `PRIMARY KEY` and `UNIQUE` constraint.
The following additional index accelerates the most frequent read path (look up all secrets
for an environment):

```sql
-- Already covered by the UNIQUE constraint index on (environment_id, key).
-- No additional indexes required for Phase 1 access patterns.
```

For Phase 3, when audit log queries by `project_id` and time range become common:

```sql
-- Future (Phase 3, additive only — does not alter existing tables)
-- CREATE INDEX IF NOT EXISTS idx_audit_logs_project_created
--     ON audit_logs(project_id, created_at DESC);
```

---

## Entity Relationships

```
projects
│  id (PK, UUID TEXT)
│  name (TEXT)
│  created_at (INTEGER)
│  updated_at (INTEGER)
│
└──< environments
      │  id (PK, UUID TEXT)
      │  project_id (FK → projects.id, CASCADE DELETE)
      │  name (TEXT, lowercase, UNIQUE per project)
      │  created_at (INTEGER)
      │  updated_at (INTEGER)
      │
      └──< secrets
            id                (PK, UUID TEXT)
            environment_id    (FK → environments.id, CASCADE DELETE)
            key               (TEXT, UNIQUE per environment)
            value_encrypted   (BLOB, AES-256-GCM ciphertext)
            value_nonce       (BLOB, 12 bytes, unique per row)
            created_at        (INTEGER)
            updated_at        (INTEGER)
```

---

## Phase 3 Extension Map (Additive Only)

The following schema additions are planned for Phase 3. None require altering existing
tables — they attach to existing UUIDs as foreign keys.

```sql
-- Phase 3: users (for team-based RBAC)
-- CREATE TABLE users (
--     id         TEXT NOT NULL PRIMARY KEY CHECK(length(id) = 36),
--     email      TEXT NOT NULL UNIQUE,
--     created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
--     updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
-- );

-- Phase 3: project membership / RBAC
-- CREATE TABLE project_members (
--     project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
--     user_id    TEXT NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
--     role       TEXT NOT NULL CHECK(role IN ('owner', 'editor', 'reader')),
--     PRIMARY KEY (project_id, user_id)
-- );

-- Phase 3: immutable audit log (secrets.id FK for traceability)
-- CREATE TABLE audit_logs (
--     id           TEXT    NOT NULL PRIMARY KEY CHECK(length(id) = 36),
--     secret_id    TEXT    REFERENCES secrets(id) ON DELETE SET NULL,
--     project_id   TEXT    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
--     actor        TEXT    NOT NULL,   -- user email or 'system'
--     action       TEXT    NOT NULL,   -- 'read' | 'write' | 'delete'
--     occurred_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
-- );
```

---

## Schema Migration Strategy

- Schema version is tracked via `PRAGMA user_version`.
- On connection open, the database module reads `user_version`:
  - If `0` (new vault): run all `CREATE TABLE IF NOT EXISTS` statements, then set
    `PRAGMA user_version = 1`.
  - If `>= 1`: apply incremental migrations only (additive ALTER TABLE or new tables).
- The developer MUST NOT run manual SQL. Migration is automatic on first use (FR-009).
