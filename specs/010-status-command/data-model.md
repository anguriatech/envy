# Data Model: Vault Sync Status Command

**Feature**: `010-status-command`
**Date**: 2026-03-25

---

## New Persistent Entity: Sync Marker

**Table**: `sync_markers`
**Purpose**: Records the Unix timestamp of the last successful `envy encrypt` operation for each environment.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `environment_id` | TEXT | NOT NULL, PRIMARY KEY, FK → `environments.id` ON DELETE CASCADE | UUID of the sealed environment |
| `sealed_at` | INTEGER | NOT NULL | Unix epoch (UTC, seconds) of the last successful seal |

**Relationships**:
- One-to-one with `environments` (at most one sync marker per environment).
- Deleted automatically when its parent environment is deleted (CASCADE).

**State transitions**:
- **Created**: First successful `envy encrypt` for this environment.
- **Updated**: Each subsequent successful `envy encrypt` for this environment (`INSERT OR REPLACE`).
- **Deleted**: When the parent environment is deleted.

---

## Computed (Transient) Entity: Environment Status Record

**Location**: Returned by `Vault::environment_status()` in the DB layer; enriched with `SyncStatus` by the Core layer.
**Not persisted** — computed on every `envy status` invocation.

| Field | Type | Source |
|-------|------|--------|
| `name` | String | `environments.name` |
| `secret_count` | i64 | `COUNT(secrets.id)` grouped by environment |
| `last_modified_at` | Option\<i64\> | `MAX(secrets.updated_at)` — `None` if secret_count == 0 |
| `sealed_at` | Option\<i64\> | `sync_markers.sealed_at` — `None` if never sealed |

---

## Computed (Transient) Entity: Sync Status

**Location**: Derived by the Core layer from `EnvironmentStatus`.
**Not persisted**.

| Value | Condition | Meaning |
|-------|-----------|---------|
| `InSync` | `sealed_at >= last_modified_at` OR (`secret_count == 0` AND `sealed_at` is Some) | No changes since last seal |
| `Modified` | `last_modified_at > sealed_at` (both Some) | Secrets changed after last seal |
| `NeverSealed` | `sealed_at` is None | Environment has never been encrypted |

**Edge case**: An environment with 0 secrets and `sealed_at = None` → `NeverSealed`.
**Edge case**: An environment with 0 secrets and `sealed_at = Some(t)` → `InSync` (the vault changed to zero secrets, but since the empty envelope was sealed, we consider it in sync from the marker's perspective — but note this case is prevented by the F1 guard in `cmd_encrypt` which skips empty environments).

---

## Computed (Transient) Entity: Artifact Metadata

**Location**: Assembled by `cmd_status` from the filesystem and `envy.enc` JSON structure.
**Not persisted**.

| Field | Type | Source |
|-------|------|--------|
| `found` | bool | `artifact_path.exists()` |
| `path` | PathBuf | Resolved artifact path |
| `last_modified_at` | Option\<i64\> | `std::fs::metadata(path)?.modified()` as Unix epoch |
| `environments` | Vec\<String\> | `SyncArtifact.environments.keys()` — no decryption |

---

## Schema Migration: V1 → V2

**Trigger**: `PRAGMA user_version == 1` on `Vault::open`.
**Action**: Execute `SCHEMA_V2` DDL, set `PRAGMA user_version = 2`.

```
V1 schema: projects, environments, secrets        (user_version = 1)
V2 schema: + sync_markers                         (user_version = 2)
```

Existing V1 vaults open cleanly; they receive an empty `sync_markers` table and every environment shows "Never Sealed" until re-encrypted.
