# Contracts: Sync Artifact Cryptography Layer

**Feature**: 005-gitops-sync-artifact
**Date**: 2026-03-22

---

## Layer Placement

All types and functions in this contract live in one of two modules, following Constitution Principle IV (strict one-way dependency):

```
src/crypto/artifact.rs   ← Pure crypto + data types (no vault, no file I/O)
src/core/sync.rs         ← Orchestration (reads vault, calls crypto, writes file)
```

Dependency direction: `src/core/sync.rs` imports from `src/crypto/artifact.rs`. Never the reverse.

---

## New Dependencies (Cargo.toml)

```toml
argon2     = "0.5"
serde_json = "1"
base64ct   = { version = "1", features = ["alloc"] }
```

---

## `src/crypto/artifact.rs` — Crypto Primitives

### Data Types

```rust
/// Root structure of an `envy.enc` file.
/// Serializes/deserializes via `serde_json`.
/// `environments` uses `BTreeMap` to guarantee alphabetical key order in JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncArtifact {
    pub version: u32,
    pub environments: BTreeMap<String, EncryptedEnvelope>,
}

/// Self-describing encrypted payload for one environment.
/// Contains everything needed to decrypt given the correct passphrase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub ciphertext: String,   // Base64-encoded AES-256-GCM ciphertext + 16-byte tag
    pub nonce: String,        // Base64-encoded 12-byte nonce
    pub kdf: KdfParams,
}

/// Argon2id key-derivation parameters embedded in every envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub algorithm: String,    // always "argon2id"
    pub memory_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub salt: String,         // Base64-encoded 16-byte random salt
}

/// Ephemeral plaintext content for one environment.
/// NEVER written to disk. Zeroed on drop via `Zeroizing` wrappers on individual values.
pub struct ArtifactPayload {
    pub secrets: BTreeMap<String, Zeroizing<String>>,
}
```

**Schema version constant**:
```rust
pub const ARTIFACT_VERSION: u32 = 1;
```

**KDF defaults**:
```rust
pub const KDF_MEMORY_KIB: u32   = 65536;  // 64 MiB
pub const KDF_TIME_COST: u32    = 3;
pub const KDF_PARALLELISM: u32  = 4;
pub const KDF_SALT_BYTES: usize = 16;
```

---

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("passphrase must not be empty or whitespace")]
    WeakPassphrase,

    #[error("envy.enc is malformed: {0}")]
    MalformedArtifact(String),

    #[error("envelope for environment '{0}' is malformed: {1}")]
    MalformedEnvelope(String, String),

    #[error("unsupported artifact version {0} (expected {1})")]
    UnsupportedVersion(u32, u32),

    #[error("key derivation failed: {0}")]
    KdfFailed(String),
}
```

---

### Function Signatures

#### `derive_key`

```rust
/// Derives a 32-byte AES-256-GCM key from `passphrase` using Argon2id with `params`.
///
/// # Arguments
/// - `passphrase`: User-provided string. MUST NOT be empty or whitespace-only.
/// - `salt`: 16-byte random salt. MUST be unique per envelope per seal operation.
/// - `params`: Argon2id cost parameters.
///
/// # Returns
/// `Ok(Zeroizing<[u8; 32]>)` — the derived key, zeroed on drop.
///
/// # Errors
/// - `ArtifactError::WeakPassphrase` if `passphrase.trim().is_empty()`.
/// - `ArtifactError::KdfFailed` on internal Argon2 failure.
pub fn derive_key(
    passphrase: &str,
    salt: &[u8; 16],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; 32]>, ArtifactError>
```

---

#### `seal_envelope`

```rust
/// Encrypts `payload` into a self-describing `EncryptedEnvelope`.
///
/// Generates a fresh 16-byte random salt (for Argon2id key derivation) and a
/// fresh 12-byte random nonce (for AES-256-GCM) using the OS CSPRNG.
/// All Argon2id parameters are embedded in the returned envelope.
///
/// # Arguments
/// - `passphrase`: Used to derive the AES-256-GCM key via Argon2id.
/// - `payload`: The secret map to encrypt. Key names are included inside the ciphertext.
///
/// # Returns
/// `Ok(EncryptedEnvelope)` — safe to serialize to `envy.enc`.
///
/// # Errors
/// - `ArtifactError::WeakPassphrase` if `passphrase.trim().is_empty()`.
/// - `ArtifactError::KdfFailed` on internal Argon2 failure.
/// - `ArtifactError::MalformedArtifact` on internal AES-GCM failure (structurally impossible
///   with valid inputs; treated as an internal error).
pub fn seal_envelope(
    passphrase: &str,
    payload: &ArtifactPayload,
) -> Result<EncryptedEnvelope, ArtifactError>
```

---

#### `unseal_envelope`

```rust
/// Decrypts an `EncryptedEnvelope` back into an `ArtifactPayload`.
///
/// Re-derives the AES-256-GCM key from `passphrase` using the Argon2id parameters
/// embedded in `envelope.kdf`, then decrypts and authenticates the ciphertext.
///
/// # Arguments
/// - `passphrase`: The passphrase used to seal this envelope.
/// - `env_name`: The environment name (used for error context only).
/// - `envelope`: The envelope to decrypt.
///
/// # Returns
/// - `Ok(ArtifactPayload)` on success.
/// - `Err(ArtifactError::WeakPassphrase)` if passphrase is empty.
/// - `Err(ArtifactError::MalformedEnvelope)` if Base64 decoding or JSON deserialization fails.
/// - `Err(ArtifactError::UnsupportedVersion)` if `kdf.algorithm` is not `"argon2id"`.
/// - `Err(ArtifactError::KdfFailed)` on internal Argon2 failure.
///
/// # Note on authentication failure
/// If AES-GCM authentication fails (wrong passphrase OR tampered ciphertext), this function
/// returns `Err(ArtifactError::MalformedEnvelope(env_name, "authentication failed"))`.
/// The caller (`unseal_artifact` in `src/core/sync.rs`) MUST treat this as a graceful skip,
/// NOT as a hard error. This is the Progressive Disclosure contract.
pub fn unseal_envelope(
    passphrase: &str,
    env_name: &str,
    envelope: &EncryptedEnvelope,
) -> Result<ArtifactPayload, ArtifactError>
```

---

## `src/core/sync.rs` — Orchestration Layer

### Result Type

```rust
/// Output of a successful `unseal_artifact` call.
pub struct UnsealResult {
    /// Successfully decrypted environments: env_name → (key → Zeroizing<value>).
    /// Values are zeroed on drop.
    pub imported: BTreeMap<String, BTreeMap<String, Zeroizing<String>>>,

    /// Environment names that could not be decrypted (wrong passphrase or malformed envelope).
    /// Not an error — callers should surface these as informational warnings.
    pub skipped: Vec<String>,
}
```

### Error Type

```rust
/// Errors at the orchestration layer (seal/unseal full artifacts + file I/O).
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Artifact(#[from] ArtifactError),

    #[error("envy.enc not found at {0}")]
    FileNotFound(String),

    #[error("failed to read/write envy.enc: {0}")]
    Io(String),

    #[error("envy.enc has unsupported schema version {0}")]
    UnsupportedVersion(u32),

    #[error("no environments could be decrypted — check your passphrase")]
    NothingImported,
}
```

---

### Function Signatures

#### `seal_artifact`

```rust
/// Reads all secrets for `envs` from the vault and seals them into a `SyncArtifact`.
///
/// Each environment is sealed independently, allowing different passphrases per
/// environment (Progressive Disclosure). If `passphrase_for_env` is `None`, `passphrase`
/// is used for all environments (Startup Mode).
///
/// # Arguments
/// - `vault`: Open vault handle (read-only during this operation).
/// - `master_key`: 32-byte master key for vault decryption.
/// - `project_id`: Identifies which project's secrets to read.
/// - `passphrase`: The shared team passphrase (Startup Mode).
/// - `envs`: If `None`, all environments with at least one secret are included.
///           If `Some(&[...])`, only the listed environment names are included.
///
/// # Returns
/// `Ok(SyncArtifact)` — ready to be serialized with `write_artifact`.
///
/// # Errors
/// - `SyncError::Artifact(ArtifactError::WeakPassphrase)` if passphrase is empty.
/// - `SyncError::Artifact(ArtifactError::KdfFailed)` on Argon2 failure.
/// - Propagates `CoreError` variants for vault read failures.
pub fn seal_artifact(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    passphrase: &str,
    envs: Option<&[&str]>,
) -> Result<SyncArtifact, SyncError>
```

---

#### `unseal_artifact`

```rust
/// Decrypts all accessible environments in `artifact` using `passphrase`.
///
/// Iterates over every environment in the artifact and attempts to decrypt each one.
/// Environments that fail authentication (wrong passphrase or malformed envelope) are
/// added to `UnsealResult.skipped` — the overall operation is NOT aborted.
///
/// Callers MUST check `result.skipped` and surface it as an informational warning.
/// If `result.imported` is empty, the caller MAY treat this as `SyncError::NothingImported`.
///
/// # Arguments
/// - `artifact`: A parsed `SyncArtifact` (from `read_artifact`).
/// - `passphrase`: The passphrase to attempt for all environments.
///
/// # Returns
/// `Ok(UnsealResult)` — even if some environments were skipped.
///
/// # Errors
/// - `SyncError::Artifact(ArtifactError::WeakPassphrase)` if passphrase is empty (checked before iteration).
/// - `SyncError::Artifact(ArtifactError::UnsupportedVersion)` if artifact.version != ARTIFACT_VERSION.
pub fn unseal_artifact(
    artifact: &SyncArtifact,
    passphrase: &str,
) -> Result<UnsealResult, SyncError>
```

---

#### `write_artifact`

```rust
/// Serializes `artifact` to JSON and writes it to `path`.
///
/// The JSON is pretty-printed with 2-space indentation for readability.
/// Keys are serialized in alphabetical order (guaranteed by `BTreeMap`).
/// Overwrites any existing file at `path`.
///
/// # Errors
/// - `SyncError::Io` on file write failure.
pub fn write_artifact(artifact: &SyncArtifact, path: &Path) -> Result<(), SyncError>
```

---

#### `read_artifact`

```rust
/// Reads and parses `envy.enc` from `path`.
///
/// Validates that the top-level `version` field equals `ARTIFACT_VERSION`.
///
/// # Errors
/// - `SyncError::FileNotFound` if `path` does not exist.
/// - `SyncError::Io` on read failure.
/// - `SyncError::Artifact(ArtifactError::MalformedArtifact)` if JSON is invalid.
/// - `SyncError::UnsupportedVersion` if `version != ARTIFACT_VERSION`.
pub fn read_artifact(path: &Path) -> Result<SyncArtifact, SyncError>
```

---

## Security Invariants

| Invariant | Where enforced |
|---|---|
| Passphrase never written to disk | `derive_key` operates in memory; result is `Zeroizing<_>` |
| Secret key names not in plaintext JSON | `ArtifactPayload` is entirely inside ciphertext |
| Nonce uniqueness | Fresh `OsRng` nonce per `seal_envelope` call |
| Salt uniqueness | Fresh `OsRng` 16-byte salt per `seal_envelope` call |
| Plaintext zeroed on drop | `ArtifactPayload.secrets` values use `Zeroizing<String>`; `UnsealResult` values use `Zeroizing<String>` |
| Vault not touched on auth failure | `unseal_artifact` returns `UnsealResult` in memory; caller (`cmd_decrypt`) writes to vault |
| No partial vault write | Caller writes all environments atomically after full in-memory unseal |

---

## stdout / stderr Contract (Core Layer)

`seal_artifact` and `unseal_artifact` produce NO terminal output. All user-facing messages (success, skipped environments, warnings) are the responsibility of the CLI layer (feature 006).

---

## Test Requirements

All functions in this contract MUST have unit tests in the same file (`#[cfg(test)]`):

| Test | Location |
|---|---|
| `derive_key` round-trip (same passphrase + salt → same key) | `src/crypto/artifact.rs` |
| `derive_key` different salts → different keys | `src/crypto/artifact.rs` |
| `seal_envelope` / `unseal_envelope` round-trip | `src/crypto/artifact.rs` |
| Wrong passphrase → `MalformedEnvelope` (graceful skip signal) | `src/crypto/artifact.rs` |
| Tampered ciphertext (bit flip) → `MalformedEnvelope` | `src/crypto/artifact.rs` |
| Empty passphrase → `WeakPassphrase` | `src/crypto/artifact.rs` |
| `seal_artifact` produces valid JSON with correct structure | `src/core/sync.rs` |
| `unseal_artifact` skips inaccessible envs (Progressive Disclosure) | `src/core/sync.rs` |
| `write_artifact` / `read_artifact` round-trip | `src/core/sync.rs` |
| Malformed JSON → `MalformedArtifact` | `src/core/sync.rs` |
| Unknown `version` → `UnsupportedVersion` | `src/core/sync.rs` |
