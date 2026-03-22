# Data Model: GitOps Sync Artifact (`envy.enc`)

**Feature**: 005-gitops-sync-artifact
**Date**: 2026-03-22

---

## Overview

The `envy.enc` artifact is the on-disk representation of all sealed environments for a project. It is a JSON file with deterministic key ordering, safe to commit to a public Git repository.

The data model has two tiers:
1. **Persistent (serialized to `envy.enc`)** — `SyncArtifact`, `EncryptedEnvelope`, `KdfParams`
2. **Ephemeral (in-memory only)** — `ArtifactPayload`, `UnsealResult`

---

## Persistent Entities (serialized to disk)

### `SyncArtifact`

The root entity. Represents the complete content of one `envy.enc` file.

| Field          | Type                                   | Constraints                                  |
|----------------|----------------------------------------|----------------------------------------------|
| `version`      | `u32`                                  | MUST be `1` for this schema version. Future readers use this to detect format migrations. |
| `environments` | `BTreeMap<String, EncryptedEnvelope>`  | Keys are environment names (lowercase). Serialized in alphabetical order. May be empty. |

**JSON example** (abbreviated):
```json
{
  "version": 1,
  "environments": {
    "development": { ... },
    "production": { ... },
    "staging": { ... }
  }
}
```

**Validation rules**:
- `version` MUST equal `1`; any other value MUST produce `SyncError::UnsupportedVersion`.
- `environments` keys MUST be non-empty strings.

---

### `EncryptedEnvelope`

The opaque, self-describing encrypted payload for one environment. Contains everything needed to decrypt the environment's secrets given the correct passphrase — no external metadata required.

| Field        | Type        | Constraints                                                   |
|--------------|-------------|---------------------------------------------------------------|
| `ciphertext` | `String`    | Base64-encoded AES-256-GCM ciphertext with 16-byte GCM tag appended. |
| `nonce`      | `String`    | Base64-encoded 12-byte (96-bit) AES-GCM nonce. Unique per envelope. |
| `kdf`        | `KdfParams` | All Argon2id parameters needed to re-derive the 32-byte encryption key from the passphrase. |

**JSON example**:
```json
{
  "ciphertext": "R2VuZXJhdGVkIGJ5IGVudnk...",
  "nonce": "YWJjZGVmZ2hpams=",
  "kdf": {
    "algorithm": "argon2id",
    "memory_kib": 65536,
    "time_cost": 3,
    "parallelism": 4,
    "salt": "c2FsdHNhbHRzYWx0c2FsdA=="
  }
}
```

**Validation rules**:
- `ciphertext` and `nonce` MUST be valid Base64. Invalid Base64 MUST produce `SyncError::MalformedEnvelope`.
- Decoded `nonce` MUST be exactly 12 bytes. Invalid length MUST produce `SyncError::MalformedEnvelope`.
- `kdf.algorithm` MUST equal `"argon2id"`. Unknown algorithm MUST produce `SyncError::UnsupportedVersion`.

---

### `KdfParams`

Argon2id key derivation parameters. Embedded in every `EncryptedEnvelope` so it is self-describing.

| Field        | Type     | Constraints                                                  |
|--------------|----------|--------------------------------------------------------------|
| `algorithm`  | `String` | MUST be `"argon2id"` for this version.                       |
| `memory_kib` | `u32`    | Argon2 memory parameter in kibibytes. Default: `65536` (64 MiB). |
| `time_cost`  | `u32`    | Argon2 time cost (iterations). Default: `3`.                 |
| `parallelism`| `u32`    | Argon2 parallelism factor. Default: `4`.                     |
| `salt`       | `String` | Base64-encoded 16-byte random salt. Unique per envelope.     |

---

## Ephemeral Entities (in-memory only, never written to disk in plaintext)

### `ArtifactPayload`

The plaintext content of one environment, serialized to JSON and then encrypted into an `EncryptedEnvelope`. It is NEVER written to disk in plaintext form.

| Field     | Type                         | Constraints                                   |
|-----------|------------------------------|-----------------------------------------------|
| `secrets` | `BTreeMap<String, String>`   | Secret key names → plaintext values. Alphabetically ordered. |

**Wire format** (the bytes that are encrypted by AES-GCM):
```json
{"API_KEY":"sk_live_abc","DATABASE_URL":"postgres://...","STRIPE_KEY":"sk_test_xyz"}
```

Key names are inside the ciphertext — not exposed in `envy.enc`. An observer cannot enumerate secret key names for a locked environment.

---

### `UnsealResult`

The return value of `unseal_artifact`. Carries both the successfully decrypted environments and a list of environments that were skipped (inaccessible).

| Field      | Type                                                    | Constraints                                              |
|------------|---------------------------------------------------------|----------------------------------------------------------|
| `imported` | `BTreeMap<String, BTreeMap<String, Zeroizing<String>>>` | Environment name → (secret key → zeroized plaintext value). |
| `skipped`  | `Vec<String>`                                           | Environment names that could not be decrypted (wrong passphrase or malformed envelope). |

**Memory safety**: All `Zeroizing<String>` values are zeroed when the `UnsealResult` is dropped. Callers MUST write secrets to the vault immediately and drop the result.

---

## Error Types

### `SyncError`

The typed error enum for all artifact-layer operations. Lives in `src/crypto/artifact.rs` (low-level) and `src/core/sync.rs` (orchestration-level).

| Variant              | Trigger                                                            | Recovery            |
|----------------------|--------------------------------------------------------------------|---------------------|
| `WeakPassphrase`     | Passphrase is empty or whitespace-only.                            | User provides non-empty passphrase. |
| `MalformedArtifact`  | `envy.enc` is not valid JSON or is missing required top-level fields. | Check file integrity / re-run `envy encrypt`. |
| `MalformedEnvelope`  | An envelope entry has invalid Base64, wrong nonce length, or unknown KDF. | Same as above.      |
| `UnsupportedVersion` | `version` field is not `1`, or `kdf.algorithm` is not `"argon2id"`. | Upgrade Envy.       |
| `FileNotFound`       | `envy.enc` does not exist at the given path.                       | Run `envy encrypt` first. |
| `IoError`            | File read or write fails (permissions, disk full, etc.).           | Operator action.    |
| `KdfFailed`          | Argon2id key derivation fails (internal error).                    | Internal; report as bug. |
| `NothingImported`    | `unseal_artifact` completes with zero imported environments.       | Surfaced as a warning, not a hard error, by the Core layer. |

---

## Entity Relationships

```
SyncArtifact
  └── environments: BTreeMap<env_name, EncryptedEnvelope>
        └── kdf: KdfParams
              (+ ciphertext + nonce)
                    │
                    │  Argon2id(passphrase, kdf.salt) → 32-byte key
                    │  AES-256-GCM.decrypt(key, nonce, ciphertext) → bytes
                    │  serde_json::from_slice(bytes) → ArtifactPayload
                    ▼
              ArtifactPayload
                └── secrets: BTreeMap<key_name, plaintext_value>

unseal_artifact(SyncArtifact, passphrase) → UnsealResult
  ├── imported: BTreeMap<env_name, BTreeMap<key, Zeroizing<value>>>
  └── skipped:  Vec<env_name>
```

---

## JSON Schema (envy.enc, Schema Version 1)

```json
{
  "$schema": "envy-sync-artifact-v1",
  "version": 1,
  "environments": {
    "<env_name>": {
      "ciphertext": "<base64-string>",
      "nonce": "<base64-12-bytes>",
      "kdf": {
        "algorithm": "argon2id",
        "memory_kib": 65536,
        "time_cost": 3,
        "parallelism": 4,
        "salt": "<base64-16-bytes>"
      }
    }
  }
}
```

All top-level and nested object keys are serialized in alphabetical order.
