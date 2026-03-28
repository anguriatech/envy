# Data Model: Pre-Encrypt Secret Diff

**Feature**: `011-envy-diff`
**Date**: 2026-03-28

---

## No New Persistent Entities

This feature introduces no new database tables or schema migrations. All data structures are transient (computed at runtime and discarded after use). The diff command is strictly read-only — it queries existing tables and reads the existing `envy.enc` artifact.

---

## Computed (Transient) Entity: Diff Entry

**Purpose**: Represents a single key-level change between the vault state and the artifact state for one environment.

| Field       | Type               | Description                                                                 |
|-------------|--------------------|-----------------------------------------------------------------------------|
| `key`       | String             | The secret key name (e.g., `DATABASE_URL`)                                  |
| `change`    | ChangeType         | One of: `Added`, `Removed`, `Modified`                                      |
| `old_value` | Option\<String\>   | Decrypted value from the artifact envelope. `None` for additions. Held in a memory-zeroed container. |
| `new_value` | Option\<String\>   | Decrypted value from the vault. `None` for deletions. Held in a memory-zeroed container.             |

**ChangeType values**:

| Value      | Condition                                               | Meaning                                   |
|------------|---------------------------------------------------------|-------------------------------------------|
| `Added`    | Key exists in vault but not in artifact                 | New secret will be sealed for the first time |
| `Removed`  | Key exists in artifact but not in vault                 | Secret was deleted from vault since last seal |
| `Modified` | Key exists in both, but decrypted values differ (bytes) | Secret value changed since last seal       |

**Note**: Keys present in both vault and artifact with identical decrypted values are excluded entirely — they do not appear in the diff.

---

## Computed (Transient) Entity: Diff Report

**Purpose**: The complete comparison result for a single environment. Returned by the Core layer to the CLI layer for rendering.

| Field        | Type               | Description                                                  |
|--------------|--------------------|--------------------------------------------------------------|
| `env_name`   | String             | Name of the compared environment (e.g., `development`)       |
| `entries`    | Vec\<DiffEntry\>   | Sorted alphabetically by `key`. Empty if no differences.     |
| `added`      | usize              | Count of `Added` entries                                     |
| `removed`    | usize              | Count of `Removed` entries                                   |
| `modified`   | usize              | Count of `Modified` entries                                  |

**Lifecycle**: Created by the Core layer's diff function, consumed by the CLI layer's renderer, then dropped. All `Zeroizing` values in the entries are zeroed on drop.

---

## Data Sources (Existing, Read-Only)

The diff command reads from two existing data sources without modifying either:

### Source A: Local Vault (via existing DB queries)

Secrets for the target environment are fetched and decrypted using the vault master key (already stored in the OS keyring). This uses the same code path as `envy list --format json`.

| Data           | Source                                               |
|----------------|------------------------------------------------------|
| Secret keys    | `secrets.key` WHERE `environment_id` matches         |
| Secret values  | `secrets.value_encrypted` + `secrets.value_nonce`, decrypted with master key |

### Source B: Artifact Envelope (via existing sync layer)

The target environment's envelope is unsealed from `envy.enc` using the passphrase. This uses the same `unseal_envelope` function as `envy decrypt`.

| Data           | Source                                               |
|----------------|------------------------------------------------------|
| Secret keys    | `ArtifactPayload.secrets` keys after unsealing       |
| Secret values  | `ArtifactPayload.secrets` values after unsealing     |

### Source Absence Rules

| Vault state         | Artifact state             | Passphrase needed? | Result                                    |
|---------------------|----------------------------|---------------------|-------------------------------------------|
| Env has secrets     | Env in artifact            | Yes                 | Full diff (additions, removals, modifications) |
| Env has secrets     | Env NOT in artifact        | No                  | All vault secrets → additions              |
| Env has secrets     | `envy.enc` missing         | No                  | All vault secrets → additions              |
| Env has NO secrets  | Env in artifact            | Yes                 | All artifact secrets → removals            |
| Env has NO secrets  | Env NOT in artifact        | No                  | No differences (empty vs empty)            |
| Env NOT in vault    | Env in artifact            | Yes                 | All artifact secrets → removals            |
| Env NOT in vault    | Env NOT in artifact        | N/A                 | Error: environment not found anywhere      |

---

## Security Invariants

1. **Memory zeroing**: All `old_value` and `new_value` fields MUST be stored in `Zeroizing<String>` containers. Backing memory is zeroed when the diff report is dropped.
2. **No disk writes**: The diff command writes nothing to disk. No temp files, no logs, no caches.
3. **Value suppression by default**: The CLI renderer MUST NOT read `old_value` or `new_value` fields from the diff entries unless `--reveal` is explicitly set. The Core layer always computes values (needed for modification detection), but the CLI layer gates their visibility.
4. **Passphrase handling**: The passphrase is used only to unseal the artifact envelope. It is not stored, logged, or passed beyond the crypto layer. Same `Zeroizing` discipline as `envy decrypt`.
