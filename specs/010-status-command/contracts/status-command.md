# Contract: `envy status` Command

**Feature**: `010-status-command`
**Date**: 2026-03-25

---

## Command Signature

```
envy status [--format <table|json>]
```

- `--format` / `-f`: Output format. Default: `table`. Only `table` and `json` are meaningful for this command; `dotenv` and `shell` are silently coerced to `table`.
- No positional arguments.
- No `--env` flag (reports all environments).

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (including: no environments, artifact not found, artifact unreadable) |
| 1 | Vault not found or not initialized |
| 4 | Vault could not be opened (wrong master key, corrupted file) |

---

## Standard Output — Table Format (default)

### Vault section header
```
Vault: ~/.envy/vault.db  (project: <project-name>)
```

### Environment table
```
Environment   Secrets   Last Modified      Status
-----------   -------   -----------------  ----------------
development       3     5 minutes ago      ⚠ Modified
production        5     2 days ago         ✓ In Sync
staging           2     Never modified     ✗ Never Sealed
```

- Columns: `Environment`, `Secrets`, `Last Modified`, `Status`
- Rows sorted alphabetically by environment name.
- `Last Modified`: relative time (e.g., "5 minutes ago"). If `secret_count == 0`, show `"No secrets"`.
- `Status` column values (exact strings):
  - `✓ In Sync` (green)
  - `⚠ Modified` (yellow)
  - `✗ Never Sealed` (red)

### Artifact section
```
Artifact: ./envy.enc  (last written: 3 hours ago)
  Sealed environments: development, production
```

Or, if not found:
```
Artifact: ./envy.enc  — not found
```

Or, if malformed:
```
Artifact: ./envy.enc  — unreadable (malformed JSON)
```

### No environments
```
No environments found. Use 'envy set' to add secrets first.
```

---

## Standard Output — JSON Format (`--format json`)

Single JSON object on stdout, followed by a newline. No other content on stdout.

```json
{
  "environments": [
    {
      "name": "development",
      "secret_count": 3,
      "last_modified_at": "2026-03-25T10:15:00Z",
      "status": "modified"
    },
    {
      "name": "production",
      "secret_count": 5,
      "last_modified_at": "2026-03-23T08:00:00Z",
      "status": "in_sync"
    },
    {
      "name": "staging",
      "secret_count": 2,
      "last_modified_at": null,
      "status": "never_sealed"
    }
  ],
  "artifact": {
    "found": true,
    "path": "./envy.enc",
    "last_modified_at": "2026-03-25T10:20:00Z",
    "environments": ["development", "production"]
  }
}
```

### JSON field contracts

**`environments[]`** (array, sorted by name):
| Field | Type | Values |
|-------|------|--------|
| `name` | string | lowercase environment name |
| `secret_count` | integer | ≥ 0 |
| `last_modified_at` | string or null | ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`) or `null` if 0 secrets |
| `status` | string | `"in_sync"`, `"modified"`, `"never_sealed"` |

**`artifact`** (object, always present):
| Field | Type | Values |
|-------|------|--------|
| `found` | boolean | `true` if `envy.enc` exists and is readable JSON, `false` otherwise |
| `path` | string | Resolved path to `envy.enc` (may or may not exist) |
| `last_modified_at` | string or null | ISO 8601 UTC of file mtime, or `null` if not found |
| `environments` | array of strings | Names present in the artifact; `[]` if not found or unreadable |

---

## Standard Error

All informational output goes to **stdout** (table or JSON). Errors only go to **stderr**:

```
error: vault not found — run 'envy init' first
```

---

## Passphrase Constraint

`envy status` MUST NOT:
- Prompt for a passphrase at any point.
- Call `unseal_envelope` or any decryption function.
- Read `ENVY_PASSPHRASE` or `ENVY_PASSPHRASE_<ENV>` environment variables.

---

## Encryption Wiring Contract

After every successful `envy encrypt` operation:
- For each environment that was sealed in that run, the vault's `sync_markers` table MUST be updated with `sealed_at = <current Unix epoch>`.
- This update MUST happen atomically with the seal operation (within the same logical transaction boundary, though not necessarily a DB transaction with `envy.enc` writing).
- If the seal fails for an environment (e.g., passphrase input error, DB error), the sync marker for that environment MUST NOT be updated.

---

## Idempotency

Running `envy status` multiple times produces identical output (given no changes to the vault or `envy.enc` between calls). It is a pure read operation.
