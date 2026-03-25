# Research: Vault Sync Status Command

**Feature**: `010-status-command`
**Date**: 2026-03-25

---

## Decision 1: ASCII Table Rendering Crate

**Decision**: Add `comfy-table = "7"` as a direct dependency.

**Rationale**:
- Clean, ergonomic API: `Table::new()` → `set_header(...)` → `add_row(...)` → `println!("{table}")`.
- Auto-computes column widths from content; no manual padding arithmetic.
- Per-cell colour support (`Cell::new(...).fg(Color::Green)`) compatible with the `console` crate already present transitively via `dialoguer`.
- Zero `unsafe` in the public API surface.
- Actively maintained; last release tracks recent Rust editions.
- MSRV compatible with Rust 1.65+, well within project's 1.85 floor.

**Alternatives considered**:
- `tabled = "0.15"`: Feature-rich but significantly larger API surface and heavier compile time for a simple 4-column table.
- `cli-table`: Less actively maintained; no meaningful advantage over `comfy-table`.
- Hand-rolling with `println!` and string padding: Viable for fixed-width content, but brittle for variable-length environment names and produces alignment bugs without a library.

---

## Decision 2: Relative Time Formatting

**Decision**: Hand-roll a `humanize_duration(secs: i64) -> String` free function inside the CLI layer (or a shared utility).

**Rationale**:
- The logic is 15–20 lines of straightforward arithmetic: compare `now - timestamp` against second/minute/hour/day thresholds and return a formatted string.
- Adding `chrono` (200 000+ lines of generated code) solely for humanize output would be disproportionate.
- The spec requires English-only output; no locale table is needed.
- `timeago` is a viable small crate (~250 lines) but introduces a dependency for logic that is trivially reproducible inline and keeps no implicit state.
- Future timestamps (sealed_at in the future due to clock skew) are handled gracefully: fall back to printing the absolute UTC date (ISO 8601 seconds portion).

**Implementation sketch** (for plan reference, not normative code):
```
diff = now_unix - timestamp
< 60 s  → "X seconds ago"
< 3600  → "X minutes ago"
< 86400 → "X hours ago"
< 604800 → "X days ago"
else   → ISO 8601 date (YYYY-MM-DD)
```

---

## Decision 3: Schema V2 Migration Strategy

**Decision**: Extend `src/db/schema.rs` with a V1→V2 step using `PRAGMA user_version = 2` and a `CREATE TABLE IF NOT EXISTS sync_markers` DDL.

**Rationale**:
- The existing migration runner already follows the `if version == N { apply_step(); set user_version = N+1; }` pattern, which is correct and idempotent.
- `CREATE TABLE IF NOT EXISTS` inside the migration step makes the step safe to re-run against an already-migrated vault (belt-and-suspenders).
- The `sync_markers` table needs only two columns: `environment_id` (TEXT PK, FK to `environments.id` with `ON DELETE CASCADE`) and `sealed_at` (INTEGER Unix epoch). No `updated_at` needed — the value is always written wholesale on each seal.
- `ALTER TABLE ADD COLUMN` is not required since `sync_markers` is a brand-new table, not a modification of an existing one.
- WAL mode has no interaction with `PRAGMA user_version`; the pragma is a simple integer header field read before WAL processing begins.

**Migration DDL** (for plan reference):
```sql
CREATE TABLE IF NOT EXISTS sync_markers (
    environment_id  TEXT    NOT NULL PRIMARY KEY
                            REFERENCES environments(id) ON DELETE CASCADE,
    sealed_at       INTEGER NOT NULL
);
```

---

## Decision 4: Sync Marker Upsert Pattern

**Decision**: Use `INSERT OR REPLACE INTO sync_markers (environment_id, sealed_at) VALUES (?1, ?2)` where `?2` is the Unix epoch at call time (not a DB default), so the caller can supply a deterministic test timestamp.

**Rationale**:
- `INSERT OR REPLACE` (equivalent to `INSERT INTO ... ON CONFLICT DO REPLACE`) is idiomatic SQLite for "create or update exactly one row keyed by PK".
- Passing the timestamp as a parameter rather than `strftime('%s', 'now')` SQL default enables deterministic unit tests that supply a fixed epoch.
- The call site in `src/core/sync.rs` (`seal_env`) supplies `std::time::SystemTime::now()` converted to a Unix epoch `i64`.

---

## Decision 5: `EnvironmentStatus` Ownership — DB vs Core layer

**Decision**: The DB layer returns a raw `EnvironmentStatus` struct (name, secret_count, last_modified_at, sealed_at); the **Core layer** computes the derived `SyncStatus` enum from those values.

**Rationale**:
- The DB layer MUST NOT contain business rules (Constitution Principle IV). The rule "modified > sealed → Modified" is a domain rule, not a persistence rule.
- The Core layer already orchestrates multi-table reads (see `get_env_secrets`). Adding a status aggregator fits naturally there.
- This separation makes the DB layer independently testable: check that the SQL returns the right raw numbers; test the SyncStatus computation separately without any DB.

---

## Decision 6: Artifact Metadata Without Decryption

**Decision**: Read the artifact's environment names by parsing the top-level JSON structure (keys of `environments` object) via `serde_json`; read the file mtime via `std::fs::metadata(path)?.modified()`.

**Rationale**:
- `SyncArtifact` is already deserializable from the `envy.enc` JSON. The environment names are plaintext JSON keys; only the envelope payloads are opaque ciphertext. Deserializing the full struct does not decrypt anything.
- `std::fs::metadata` requires no additional dependency.
- This satisfies FR-003: the status command never calls `unseal_envelope` or reads any passphrase.

---

## Decision 7: JSON Output Shape

**Decision**: `cmd_status` with `--format json` serializes a standalone `StatusReport` struct (not added to the existing `OutputData` enum, which models per-secret operations).

**Rationale**:
- `OutputData` was designed for single-environment key/value output. The status report is multi-environment and includes artifact metadata — a structurally different payload.
- `cmd_status` calls `serde_json::to_writer` directly, consistent with how other commands serialize JSON (via the `fmt_json` path in `format.rs`).
- The `StatusReport` struct lives in `src/cli/commands.rs` as a private serde struct (same pattern as `ListJson`, `ItemJson` in `format.rs`).
