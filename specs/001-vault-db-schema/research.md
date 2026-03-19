# Research: Vault Core Data Model

**Feature**: 001-vault-db-schema
**Date**: 2026-03-18

---

## Decision 1: UUID Storage Format (TEXT vs BLOB)

**Decision**: Store UUIDs as `TEXT` in the canonical hyphenated format
(`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`).

**Rationale**: SQLite has no native UUID type. BLOB (16 bytes) is marginally more compact,
but TEXT is human-readable in SQLite browsers, avoids endianness issues during export, and
makes future sync/debugging trivial. The performance delta is negligible for a
single-user local vault with at most tens of thousands of rows.

**Alternatives considered**:
- BLOB (16 bytes): Compact but opaque; rejected for debuggability.
- INTEGER autoincrement: Rejected — cannot guarantee global uniqueness across machines
  for Phase 2 sync.

---

## Decision 2: Timestamp Format (TEXT ISO-8601 vs INTEGER Unix epoch)

**Decision**: Store timestamps as `INTEGER` (Unix epoch in seconds, UTC).

**Rationale**: INTEGER is natively supported by SQLite's date functions
(`datetime(ts, 'unixepoch')`), sorts correctly without string parsing, and uses 8 bytes
vs ~20 for a TEXT ISO-8601 string. The spec allows both; INTEGER is strictly better for
a programmatic tool.

**Alternatives considered**:
- TEXT ISO-8601: Human-readable but larger and requires parsing for arithmetic. Rejected.
- REAL (Julian day): Less intuitive for developers; rejected.

---

## Decision 3: Defense-in-Depth Secret Value Encryption

**Decision**: Store each secret value as two columns: `value_encrypted` (BLOB, AES-256-GCM
ciphertext) and `value_nonce` (BLOB, 12-byte random nonce). The encryption key is derived
per-secret from the master key + secret UUID (HKDF-SHA256).

**Rationale**:
- SQLCipher provides full-file AES-256 encryption (first layer). This alone satisfies
  Principle I if the master key is strong.
- A second layer (per-secret AES-256-GCM) provides defense-in-depth: if the DB file
  encryption is somehow bypassed or a future SQL injection occurs, individual secret values
  remain ciphertext.
- A per-secret nonce ensures identical values produce different ciphertexts.
- HKDF key derivation means the Core layer never reuses the raw master key as a per-row
  encryption key, following cryptographic best practices.

**Alternatives considered**:
- Store plaintext in the already-encrypted DB: Rejected per constitution Principle I
  (defense-in-depth required).
- Per-row random key stored in a separate table: Adds complexity with no security benefit
  over HKDF; rejected.
- Encrypting only "high-sensitivity" fields: Rejected — "sensitivity" cannot be reliably
  determined by the schema layer.

---

## Decision 4: Foreign Key Enforcement

**Decision**: Enable `PRAGMA foreign_keys = ON` at every connection open.

**Rationale**: SQLite does NOT enforce foreign keys by default — this must be set per
connection. The database module MUST set this pragma before any query. Without it, cascade
deletes and referential integrity (FR-006, FR-007, FR-008) are silently unenforced.

**Alternatives considered**:
- Application-level cascade: Fragile — any direct SQL execution bypasses it. Rejected.

---

## Decision 5: SQLite Journal Mode

**Decision**: Set `PRAGMA journal_mode = WAL` (Write-Ahead Logging) at connection open.

**Rationale**: WAL allows concurrent readers while a write is in progress, preventing
read-lock contention when the CLI's `run` command reads env vars while another process
is writing. It also provides better crash recovery than the default DELETE journal mode.

**Alternatives considered**:
- Default DELETE journal: Blocks all readers during writes. Rejected for `run` use case.
- MEMORY journal: No crash safety. Rejected per Principle I (data integrity).

---

## Decision 6: UNIQUE Constraint Placement

**Decision**:
- `environments`: `UNIQUE(project_id, name)` — one environment name per project.
- `secrets`: `UNIQUE(environment_id, key)` — one value per key per environment.

**Rationale**: These constraints are the database-level enforcement of FR-002 and FR-003.
An `INSERT OR REPLACE` (upsert) strategy against these constraints implements the
"overwrite on conflict" behavior for `envy set` without requiring a separate SELECT first.

---

## Decision 7: Phase 3 Extensibility Anchors

**Decision**: Design the three tables with explicit extension points for Phase 3, without
adding columns that would be unused in Phase 1:

- `projects` — will accept a future `owner_id TEXT REFERENCES users(id)` FK.
- `environments` — will accept a future `is_protected INTEGER DEFAULT 0` flag for RBAC.
- `secrets` — `id` (UUID) serves as the stable FK target for future `audit_log` entries.
  A future `version INTEGER DEFAULT 1` column supports secret rotation history.

**Rationale**: The UUID primary keys on all three tables ensure that a future `audit_logs`
table can reference `secrets.id`, `environments.id`, or `projects.id` without schema
changes to existing tables (FR-010). Phase 3 additions are additive only.
