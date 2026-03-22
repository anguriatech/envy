# Implementation Plan: GitOps Sync Artifact (`envy.enc`)

**Branch**: `005-gitops-sync-artifact` | **Date**: 2026-03-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-gitops-sync-artifact/spec.md`

---

## Summary

Implement the `envy.enc` sync artifact: a self-describing, environment-keyed JSON file where each environment's secrets are independently encrypted with AES-256-GCM using an Argon2id-derived key. The module spans two layers — `src/crypto/artifact.rs` (pure crypto primitives + data types) and `src/core/sync.rs` (orchestration: reads vault, calls crypto, reads/writes file). Progressive Disclosure is implemented by attempting each environment independently in `unseal_artifact` and adding authentication failures to a `skipped` list rather than aborting.

---

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.85
**Primary Dependencies**: `argon2 = "0.5"`, `serde_json = "1"`, `base64ct = { version = "1", features = ["alloc"] }` (new); existing: `aes-gcm`, `zeroize`, `serde`, `thiserror`
**Storage**: `envy.enc` (JSON file in project root, written by `write_artifact`, read by `read_artifact`)
**Testing**: `cargo test` — unit tests in `src/crypto/artifact.rs` and `src/core/sync.rs`; integration test using `tempfile`
**Target Platform**: Linux / macOS / Windows (same as Phase 1)
**Project Type**: CLI tool (library + binary crate)
**Performance Goals**: `seal_artifact` for a typical project (≤50 secrets, 3 environments) MUST complete in under 5 seconds on commodity hardware (Argon2id at 64 MiB × 3 environments = ~1.5 s, dominated by KDF)
**Constraints**: Zero plaintext secrets on disk; all decrypted values `Zeroizing<_>`; no `.unwrap()` without inline justification
**Scale/Scope**: Single project artifact; designed for teams of 1–20

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Security by Default | ✓ PASS | Plaintext secrets exist only in `Zeroizing<_>` memory during `unseal_envelope`; never written to disk. Vault untouched on auth failure. |
| II. Determinism | ✓ PASS | `SyncArtifact` JSON uses `BTreeMap` → alphabetical key order, stable across platforms. Randomness (nonce, salt) is explicitly from `OsRng` (CSPRNG). |
| III. Rust Best Practices | ✓ PASS | All functions return `Result<T, E>`; typed error enums (`ArtifactError`, `SyncError`); no `.unwrap()` without justification; unit tests mandatory per contract. |
| IV. Modularity | ✓ PASS | `src/crypto/artifact.rs` has no imports from Core/CLI/DB. `src/core/sync.rs` imports from Crypto only. One-way dependency maintained. |
| V. Language | ✓ PASS | All identifiers, comments, and docs are in English. |
| Tech Stack | ✓ PASS | `argon2` and `base64ct` are RustCrypto ecosystem crates (mandated by constitution). `serde_json` is universally audited. Custom crypto primitives: none. |

No violations. No Complexity Tracking required.

---

## Project Structure

### Documentation (this feature)

```text
specs/005-gitops-sync-artifact/
├── spec.md                        # Feature specification
├── plan.md                        # This file
├── research.md                    # Technical decisions (Phase 0)
├── data-model.md                  # Entity definitions (Phase 1)
├── contracts/
│   └── crypto-artifact.md         # Function signatures and contracts (Phase 1)
└── tasks.md                       # Execution tasks (NOT created by /speckit.plan)
```

### Source Code

```text
src/
├── crypto/
│   ├── mod.rs          ← add pub use artifact::{...}
│   ├── aead.rs         ← unchanged
│   ├── artifact.rs     ← NEW: SyncArtifact, EncryptedEnvelope, KdfParams,
│   │                         ArtifactPayload, ArtifactError,
│   │                         derive_key, seal_envelope, unseal_envelope
│   ├── error.rs        ← unchanged
│   └── keyring.rs      ← unchanged
├── core/
│   ├── mod.rs          ← add pub use sync::{...}
│   ├── ops.rs          ← unchanged
│   ├── manifest.rs     ← unchanged
│   ├── error.rs        ← add SyncError variant (or keep SyncError separate)
│   └── sync.rs         ← NEW: UnsealResult, SyncError,
│                             seal_artifact, unseal_artifact,
│                             write_artifact, read_artifact
tests/
└── sync_artifact.rs    ← NEW: integration tests (tempfile-based)
```

**Structure Decision**: Single project layout. Two new files added to the existing `src/crypto/` and `src/core/` modules. No new modules or crates. Strict one-way dependency maintained.

---

## Phase 0: Research (Complete)

See [research.md](research.md) for all decisions. Summary:

| Decision | Choice |
|---|---|
| KDF algorithm | Argon2id (RFC 9106) |
| KDF parameters | memory 64 MiB, time 3, parallelism 4, salt 16 bytes |
| AEAD | AES-256-GCM (reuses existing `aes-gcm` crate) |
| Base64 | `base64ct` standard alphabet, no padding |
| JSON library | `serde_json` with `BTreeMap` for deterministic ordering |
| Inner plaintext format | `BTreeMap<String, Zeroizing<String>>` → JSON bytes |
| Progressive Disclosure | Any AES-GCM auth failure → graceful skip (cannot distinguish wrong key from tampered) |
| Memory safety | All decrypted values `Zeroizing<_>` |

---

## Phase 1: Design & Contracts (Complete)

All design artifacts generated. See:
- [data-model.md](data-model.md) — `SyncArtifact`, `EncryptedEnvelope`, `KdfParams`, `ArtifactPayload`, `UnsealResult`, `ArtifactError`, `SyncError`
- [contracts/crypto-artifact.md](contracts/crypto-artifact.md) — All function signatures, security invariants, and test requirements

---

## Phase 2: Implementation Algorithms

### `derive_key(passphrase, salt, params) → Zeroizing<[u8; 32]>`

```
1. If passphrase.trim().is_empty() → return Err(WeakPassphrase)
2. Build Argon2id context from params (memory_kib, time_cost, parallelism)
3. Call argon2::Argon2::hash_password_into(passphrase.as_bytes(), salt, &mut output[..32])
4. Wrap output in Zeroizing<[u8; 32]> and return Ok
```

### `seal_envelope(passphrase, payload) → EncryptedEnvelope`

```
1. If passphrase.trim().is_empty() → return Err(WeakPassphrase)
2. Generate salt: OsRng.fill_bytes(&mut salt[..16])
3. Build KdfParams { algorithm: "argon2id", memory_kib: KDF_MEMORY_KIB, time_cost: KDF_TIME_COST,
                     parallelism: KDF_PARALLELISM, salt: base64ct::encode(salt) }
4. Call derive_key(passphrase, &salt, &kdf_params) → key
5. Serialize payload.secrets (BTreeMap) to JSON bytes via serde_json
6. Call existing crypto::encrypt(&key, &json_bytes) → EncryptedSecret { ciphertext, nonce }
7. Return EncryptedEnvelope {
       ciphertext: base64ct::encode(ciphertext),
       nonce: base64ct::encode(nonce),
       kdf: kdf_params,
   }
```

### `unseal_envelope(passphrase, env_name, envelope) → ArtifactPayload`

```
1. If passphrase.trim().is_empty() → return Err(WeakPassphrase)
2. If envelope.kdf.algorithm != "argon2id" → return Err(UnsupportedVersion(…))
3. Decode envelope.salt from Base64 → salt: [u8; 16]  (Err → MalformedEnvelope)
4. Call derive_key(passphrase, &salt, &envelope.kdf) → key
5. Decode envelope.nonce from Base64 → nonce_bytes  (Err → MalformedEnvelope)
6. Decode envelope.ciphertext from Base64 → ct_bytes  (Err → MalformedEnvelope)
7. Call existing crypto::decrypt(&key, &ct_bytes, &nonce_bytes)
   → on Err(DecryptionFailed) → return Err(MalformedEnvelope(env_name, "authentication failed"))
8. Deserialize plaintext bytes as BTreeMap<String, String> via serde_json
   → on Err → return Err(MalformedEnvelope(env_name, "payload JSON invalid"))
9. Wrap values in Zeroizing<String>
10. Return Ok(ArtifactPayload { secrets })
```

### `seal_artifact(vault, master_key, project_id, passphrase, envs) → SyncArtifact`

```
1. Validate passphrase (non-empty) → Err(WeakPassphrase) early
2. Determine env_names:
   - If envs is Some(&[...]) → use those names
   - If envs is None → call vault.list_environments(project_id) to get all env names
3. For each env_name in env_names:
   a. Call core::get_env_secrets(vault, master_key, project_id, env_name) → secrets_map
      (empty map is fine — produces an envelope with zero secrets)
   b. Build ArtifactPayload { secrets: secrets_map }
   c. Call seal_envelope(passphrase, &payload) → EncryptedEnvelope
   d. Insert into environments BTreeMap: env_name → envelope
4. Return Ok(SyncArtifact { version: ARTIFACT_VERSION, environments })
```

### `unseal_artifact(artifact, passphrase) → UnsealResult`

```
1. If passphrase.trim().is_empty() → return Err(WeakPassphrase)
2. If artifact.version != ARTIFACT_VERSION → return Err(UnsupportedVersion(artifact.version))
3. Let imported = BTreeMap::new(); skipped = Vec::new()
4. For each (env_name, envelope) in &artifact.environments (alphabetical, BTreeMap):
   a. Match unseal_envelope(passphrase, env_name, envelope):
      - Ok(payload) → insert payload.secrets into imported[env_name]
      - Err(_)      → push env_name to skipped  (ALL errors → graceful skip)
5. Return Ok(UnsealResult { imported, skipped })
```

### `write_artifact(artifact, path) → ()`

```
1. serde_json::to_string_pretty(artifact) → json_string  (Err → SyncError::Io)
2. fs::write(path, json_string.as_bytes())  (Err → SyncError::Io)
```

### `read_artifact(path) → SyncArtifact`

```
1. If !path.exists() → return Err(FileNotFound(path.display()))
2. fs::read_to_string(path) → content  (Err → SyncError::Io)
3. serde_json::from_str::<SyncArtifact>(&content)  (Err → SyncError::Artifact(MalformedArtifact))
4. If artifact.version != ARTIFACT_VERSION → return Err(UnsupportedVersion(artifact.version))
5. Return Ok(artifact)
```

---

## Memory Safety Table

| Sensitive data | Type | Zeroed by |
|---|---|---|
| Derived AES key | `Zeroizing<[u8; 32]>` | Drop (automatic) |
| Serialized plaintext (JSON bytes) | Constructed in `seal_envelope`, passed to `encrypt` by reference | `encrypt` consumes; underlying Vec dropped immediately after |
| Decrypted plaintext bytes | `Zeroizing<Vec<u8>>` (returned by existing `crypto::decrypt`) | Drop (automatic) |
| `ArtifactPayload` values | `Zeroizing<String>` | Drop (automatic) |
| `UnsealResult` values | `Zeroizing<String>` | Drop (automatic) |

---

## Error Propagation Table

| Source error | Becomes |
|---|---|
| `ArtifactError::WeakPassphrase` | Propagated as-is via `SyncError::Artifact(#[from])` |
| `ArtifactError::MalformedEnvelope` (in `unseal_artifact`) | **Caught** → graceful skip (NOT propagated) |
| `ArtifactError::MalformedArtifact` (in `read_artifact`) | Propagated as `SyncError::Artifact(...)` |
| `ArtifactError::UnsupportedVersion` | Propagated as `SyncError::UnsupportedVersion` |
| `std::io::Error` | Wrapped as `SyncError::Io(e.to_string())` |
| `CoreError` (from `get_env_secrets`) | Propagated as `SyncError::Core(...)` |

---

## Post-Design Constitution Re-Check

All four principles remain satisfied after the full design:

- **Principle I**: The plaintext payload is `Zeroizing`-wrapped at every step from decryption through the `UnsealResult`. The vault is never modified until the caller (CLI layer) explicitly writes after a successful unseal.
- **Principle II**: `BTreeMap` serialization order is deterministic. Randomness (salt, nonce) is from `OsRng` and explicitly documented.
- **Principle III**: All new functions return `Result`. No `.unwrap()` planned without justification. Test matrix defined in contracts.
- **Principle IV**: `src/crypto/artifact.rs` has zero imports from Core, CLI, or DB. `src/core/sync.rs` imports from Crypto only via `use crate::crypto::artifact::*`.