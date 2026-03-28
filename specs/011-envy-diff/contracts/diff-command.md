# Contract: `envy diff` Command

**Feature**: `011-envy-diff`
**Date**: 2026-03-28

---

## Command Signature

```
envy diff [-e <ENV>] [--reveal] [--format <table|json>]
```

- `-e` / `--env`: Target environment name. Default: `development`.
- `--reveal`: Include plaintext secret values in the output. Off by default.
- `--format` / `-f`: Output format. Default: `table`. Only `table` and `json` are meaningful for this command; `dotenv` and `shell` are silently coerced to `table`.
- No positional arguments.

---

## Exit Codes

| Code | Meaning                                                                       |
|------|-------------------------------------------------------------------------------|
| 0    | Success, no differences between vault and artifact for the target environment |
| 1    | Success, differences found (additions, deletions, or modifications exist)     |
| 2    | Authentication error (wrong passphrase for the artifact envelope)             |
| 3    | Environment not found (neither vault nor artifact contains the environment)   |
| 4    | Vault could not be opened (wrong master key, corrupted file)                  |
| 5    | Artifact unreadable (exists but is malformed JSON or unsupported version)     |

**Note**: Exit code 1 for "differences found" follows the `diff(1)` and `git diff --exit-code` convention. Scripts can use `envy diff && echo "clean" || echo "drift detected"`.

---

## Passphrase Resolution

The command resolves the passphrase for the target environment using the same priority order as `envy encrypt`:

1. Environment-specific variable: `ENVY_PASSPHRASE_<ENV>` (e.g., `ENVY_PASSPHRASE_PRODUCTION`)
2. Global variable: `ENVY_PASSPHRASE`
3. Interactive TTY prompt (hidden input, no confirmation — this is a read-only operation)

**When no passphrase is needed** (artifact missing or target environment absent from artifact): steps 1–3 are skipped entirely. No prompt, no env var read.

---

## Standard Output — Table Format (default)

### Differences found

```
envy diff: development (vault ↔ envy.enc)

  + NEW_API_KEY
  - DEPRECATED_TOKEN
  ~ DATABASE_URL

3 changes: 1 added, 1 removed, 1 modified
```

- Prefix symbols: `+` (added), `-` (removed), `~` (modified).
- Colors: green (`+`), red (`-`), yellow (`~`). Suppressed when stdout is not a TTY or `NO_COLOR` is set.
- Entries sorted alphabetically by key name.
- Summary line at the bottom with counts.
- Header line identifies the environment and the two sides being compared.

### Differences found with `--reveal`

```
⚠ Warning: secret values are visible in the output below.

envy diff: development (vault ↔ envy.enc)

  + NEW_API_KEY
    vault:    sk_live_abc123

  - DEPRECATED_TOKEN
    artifact: eyJhbGciOi...

  ~ DATABASE_URL
    artifact: postgres://old-host:5432/db
    vault:    postgres://new-host:5432/db

3 changes: 1 added, 1 removed, 1 modified
```

- Warning line printed to **stderr** (not stdout) before any output.
- For additions: `vault:` line shows the new value; no `artifact:` line.
- For deletions: `artifact:` line shows the old value; no `vault:` line.
- For modifications: both `artifact:` (old) and `vault:` (new) lines shown.
- Value lines are indented for visual grouping.

### No differences

```
envy diff: development — no differences
```

### Artifact not found (all additions)

```
envy diff: development (vault ↔ envy.enc)
Note: envy.enc not found — all vault secrets shown as additions.

  + API_KEY
  + DATABASE_URL
  + SECRET_TOKEN

3 changes: 3 added, 0 removed, 0 modified
```

### Environment not in artifact (all additions)

```
envy diff: staging (vault ↔ envy.enc)
Note: environment 'staging' not found in envy.enc — all vault secrets shown as additions.

  + REDIS_URL

1 change: 1 added, 0 removed, 0 modified
```

### Environment not in vault (all deletions)

```
envy diff: production (vault ↔ envy.enc)
Note: environment 'production' not found in vault — all artifact secrets shown as deletions.

  - DB_PASSWORD
  - API_SECRET

2 changes: 0 added, 2 removed, 0 modified
```

### Error: environment not found anywhere

```
error: environment 'foo' not found in vault or artifact
```

(Printed to stderr, exit code 3)

---

## Standard Output — JSON Format (`--format json`)

Single JSON object on stdout, followed by a newline. No other content on stdout.

### Without `--reveal`

```json
{
  "environment": "development",
  "has_differences": true,
  "summary": {
    "added": 1,
    "removed": 1,
    "modified": 1,
    "total": 3
  },
  "changes": [
    {
      "key": "DATABASE_URL",
      "type": "modified"
    },
    {
      "key": "DEPRECATED_TOKEN",
      "type": "removed"
    },
    {
      "key": "NEW_API_KEY",
      "type": "added"
    }
  ]
}
```

### With `--reveal`

```json
{
  "environment": "development",
  "has_differences": true,
  "summary": {
    "added": 1,
    "removed": 1,
    "modified": 1,
    "total": 3
  },
  "changes": [
    {
      "key": "DATABASE_URL",
      "type": "modified",
      "old_value": "postgres://old-host:5432/db",
      "new_value": "postgres://new-host:5432/db"
    },
    {
      "key": "DEPRECATED_TOKEN",
      "type": "removed",
      "old_value": "eyJhbGciOi...",
      "new_value": null
    },
    {
      "key": "NEW_API_KEY",
      "type": "added",
      "old_value": null,
      "new_value": "sk_live_abc123"
    }
  ]
}
```

### No differences

```json
{
  "environment": "development",
  "has_differences": false,
  "summary": {
    "added": 0,
    "removed": 0,
    "modified": 0,
    "total": 0
  },
  "changes": []
}
```

### JSON field contracts

**Root object**:

| Field              | Type    | Description                                          |
|--------------------|---------|------------------------------------------------------|
| `environment`      | string  | Name of the compared environment                     |
| `has_differences`  | boolean | `true` if changes array is non-empty                 |
| `summary`          | object  | Counts of each change type                           |
| `changes`          | array   | Sorted alphabetically by `key`                       |

**`summary`**:

| Field      | Type    | Description                      |
|------------|---------|----------------------------------|
| `added`    | integer | Number of keys added (≥ 0)       |
| `removed`  | integer | Number of keys removed (≥ 0)     |
| `modified` | integer | Number of keys modified (≥ 0)    |
| `total`    | integer | Sum of added + removed + modified |

**`changes[]`** (always present, may be empty):

| Field       | Type           | When present                     | Description                                  |
|-------------|----------------|----------------------------------|----------------------------------------------|
| `key`       | string         | Always                           | Secret key name                              |
| `type`      | string         | Always                           | `"added"`, `"removed"`, or `"modified"`      |
| `old_value` | string or null | Only when `--reveal` is set      | Value from artifact (`null` for additions)   |
| `new_value` | string or null | Only when `--reveal` is set      | Value from vault (`null` for deletions)      |

**Note**: `old_value` and `new_value` fields are completely absent (not `null`) when `--reveal` is not set. This prevents accidental exposure through JSON key enumeration.

---

## Standard Error

- Authentication errors: `error: incorrect passphrase for environment 'development'`
- Missing environment: `error: environment 'foo' not found in vault or artifact`
- Malformed artifact: `error: envy.enc is unreadable (malformed JSON)`
- Reveal warning: `⚠ Warning: secret values are visible in the output below.` (stderr, only with `--reveal`)

All informational output goes to **stdout**. Only errors and the reveal warning go to **stderr**.

---

## Idempotency

Running `envy diff` multiple times produces identical output (given no changes to the vault or `envy.enc` between calls). It is a pure read operation — it modifies nothing.

---

## Security Contract

1. **Default safe**: Without `--reveal`, zero secret values appear in stdout or stderr. The `old_value`/`new_value` JSON fields are entirely absent (not set to `null` or `"***"`).
2. **Explicit opt-in**: The `--reveal` flag is the sole mechanism to include values. No environment variable, config file, or alias can substitute for it.
3. **Stderr warning**: When `--reveal` is active, a warning is printed to stderr before any output, alerting the user that secret values follow.
4. **Memory discipline**: All decrypted values (from both vault and artifact) are held in memory-zeroed containers and dropped immediately after comparison or rendering.
5. **No side effects**: The command writes nothing to disk. No temp files, no logs, no sync marker updates.
