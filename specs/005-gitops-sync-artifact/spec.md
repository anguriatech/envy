# Feature Specification: GitOps Sync Artifact (`envy.enc`)

**Feature Branch**: `005-gitops-sync-artifact`
**Created**: 2026-03-22
**Status**: Draft
**Input**: User description: "005-sync-artifact: Generate, structure, and encrypt the envy.enc GitOps artifact. Handles envelope JSON structure with per-environment encrypted payloads, Argon2 key derivation from user passphrase, AES-256-GCM authenticated encryption, Progressive Disclosure (graceful skip of environments the user lacks the key for), and fail-fast integrity validation. Does NOT include CLI commands (encrypt/decrypt) — those are Phase 2 CLI layer."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Seal the Local Vault into a Shareable Artifact (Priority: P1)

A team lead has finished configuring secrets across `development`, `staging`, and `production` environments in their local vault. They want to package all environments into a single encrypted file (`envy.enc`) that their team can safely commit to the Git repository without exposing any plaintext.

**Why this priority**: This is the fundamental produce operation of the entire GitOps model. Without reliably generating `envy.enc`, the collaboration workflow cannot begin.

**Independent Test**: Can be fully tested by calling `seal_artifact` with a vault populated across multiple environments and verifying a valid, opaque `envy.enc` JSON file is produced — with no plaintext visible in its contents.

**Acceptance Scenarios**:

1. **Given** a local vault with secrets in `development`, `staging`, and `production`, and a user-provided passphrase, **When** `seal_artifact` is called, **Then** a well-formed `envy.enc` JSON file is produced with one encrypted entry per environment, and no plaintext values appear anywhere in the output.
2. **Given** a vault with only one environment populated (`development`), **When** `seal_artifact` is called, **Then** the artifact contains exactly one entry for `development`; other environments are absent from the file.
3. **Given** an empty vault (no secrets), **When** `seal_artifact` is called, **Then** the artifact is produced successfully with an empty environments map and no error is returned.

---

### User Story 2 — Fully Unseal an Artifact with the Correct Passphrase (Priority: P1)

A developer clones the repository and runs the decrypt command. They have been given the shared team passphrase via their password manager. They expect all environments to be decrypted and returned in a single operation, ready to be written into their local vault.

**Why this priority**: This is the fundamental consume operation. A developer who cannot unseal the artifact cannot work with shared secrets.

**Independent Test**: Can be fully tested by sealing a known set of secrets into an artifact, then calling `unseal_artifact` with the same passphrase, and asserting the decrypted values match the original input byte-for-byte.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` artifact sealed with passphrase `"correct-horse-battery-staple"`, **When** `unseal_artifact` is called with the same passphrase, **Then** all environments and their secrets are returned as plaintext key-value maps, matching the original input exactly.
2. **Given** an artifact sealed with one passphrase, **When** `unseal_artifact` is called with a wrong passphrase, **Then** the operation skips all environments gracefully and returns them in the `skipped` list — no data is written to the vault.

---

### User Story 3 — Progressive Disclosure: Graceful Skip of Inaccessible Environments (Priority: P2)

A junior developer is given a passphrase that only unlocks `development` and `staging` (sealed with a different passphrase from `production`). When they unseal the artifact, they successfully import the environments they have access to, and receive a clear notification that `production` was skipped — without an error that halts the process.

**Why this priority**: This is the key differentiator for Enterprise Mode. It enables least-privilege access by design, where the system degrades gracefully rather than failing when some environments are locked with different keys.

**Independent Test**: Can be tested by sealing an artifact where `development` and `production` use different passphrases, calling `unseal_artifact` with the `development` passphrase, and asserting: (a) `development` secrets are returned correctly, (b) `production` appears in the skipped list, (c) no error is returned.

**Acceptance Scenarios**:

1. **Given** an artifact with `development` sealed with `"dev-key"` and `production` sealed with `"prod-key"`, **When** `unseal_artifact` is called with `"dev-key"`, **Then** `development` secrets are returned successfully and `production` is listed as skipped (not as an error).
2. **Given** an artifact with multiple environments all locked with different keys, **When** `unseal_artifact` is called with a key matching none of them, **Then** all environments appear in the skipped list, a warning is surfaced, and no error is returned.
3. **Given** `unseal_artifact` skips an environment, **Then** the local vault is NOT modified for that environment — its previous contents remain intact.

---

### User Story 4 — Tamper Detection: Fail Fast on Invalid Ciphertext (Priority: P1)

A security-conscious team lead wants confidence that if any byte of `envy.enc` is edited, corrupted in transit, or tampered with, the system immediately rejects the artifact before touching the local vault.

**Why this priority**: Authenticated encryption is the primary integrity guarantee of the artifact. Silently importing corrupted data would be a critical security failure.

**Independent Test**: Can be tested by sealing a known artifact, flipping a single byte in the Base64 ciphertext payload, and asserting that `unseal_artifact` skips that environment and performs zero writes to the vault.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` file where the ciphertext payload for one environment has been altered, **When** `unseal_artifact` is called with the correct passphrase, **Then** authentication fails for that environment, it is added to the skipped list, and no secrets are written to the vault.
2. **Given** an artifact where the nonce field has been modified, **When** `unseal_artifact` is called, **Then** authentication fails and the operation skips that environment without writing anything.
3. **Given** a structurally invalid `envy.enc` (malformed JSON), **When** `unseal_artifact` is called, **Then** the operation returns a `MalformedArtifact` error immediately, before any decryption is attempted.

---

### Edge Cases

- What happens when the passphrase is an empty string or whitespace only? → Rejected with a `WeakPassphrase` error before any cryptographic operation begins.
- What happens when `envy.enc` does not exist at the given path? → Returns a `FileNotFound` error immediately; the vault is not touched.
- What happens when two environments in the artifact share the same name (malformed file)? → The later entry overwrites the earlier one during JSON parsing; environment names are treated as unique keys.
- What happens if the Argon2 parameters in the artifact have been tampered with? → The derived key will be wrong, causing AES-GCM authentication to fail at the ciphertext layer — the AEAD tag detects the discrepancy without needing a separate integrity check on the parameters.
- What happens if the local vault is read-only during an unseal? → After successful in-memory decryption, the vault write fails with an `IoError`. The vault is left unmodified.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST represent `envy.enc` as a JSON file where top-level keys are environment names and each value is a self-contained encrypted payload — making environment-level Git diff and merge conflict resolution straightforward.
- **FR-002**: Each encrypted payload MUST be self-describing: it MUST embed the Base64-encoded ciphertext (with the authentication tag), the Base64-encoded nonce, and all key-derivation parameters (algorithm, memory cost, time cost, parallelism, salt) so it can be decrypted without any external metadata.
- **FR-003**: The system MUST derive the encryption key from a user-provided passphrase using Argon2id, producing a 256-bit key. The salt MUST be randomly generated per environment per seal operation.
- **FR-004**: The system MUST use AES-256-GCM (authenticated encryption) to encrypt and decrypt environment payloads, ensuring both confidentiality and integrity in a single pass.
- **FR-005**: The `seal_artifact` operation MUST encrypt each environment independently, so that different environments MAY use different passphrases (Progressive Disclosure).
- **FR-006**: The `unseal_artifact` operation MUST attempt each environment independently. If authentication fails for a specific environment (wrong passphrase or corrupted ciphertext), that environment MUST be added to a `skipped` list — the overall operation MUST NOT abort.
- **FR-007**: The `unseal_artifact` operation MUST NOT write any data to the local vault until all targeted environments have been successfully decrypted in memory. Any subsequent IO failure MUST leave the vault in its pre-operation state.
- **FR-008**: The `envy.enc` file MUST contain zero plaintext secret values, zero plaintext key names, and zero project identifiers — it MUST be safe to commit to a public Git repository.
- **FR-009**: The artifact format MUST include a top-level schema version field (e.g., `"version": 1`) to enable future format migrations without breaking backward compatibility.
- **FR-010**: The system MUST reject an empty or whitespace-only passphrase before attempting any cryptographic operation, returning a `WeakPassphrase` error.
- **FR-011**: The plaintext content encrypted per environment MUST include both the secret key names and their values. Key names MUST NOT appear in the plaintext JSON envelope to prevent metadata leakage.
- **FR-012**: JSON key ordering within the artifact MUST be alphabetical (environments map, and fields within each envelope) to produce stable, deterministic output across platforms for clean Git diffs.

### Key Entities

- **`SyncArtifact`**: The top-level structure of `envy.enc`. Contains a schema version integer and an alphabetically-ordered map of environment name → `EncryptedEnvelope`.
- **`EncryptedEnvelope`**: A self-contained encrypted blob for one environment. Carries the Base64 ciphertext, Base64 nonce, and all Argon2 parameters needed to re-derive the decryption key from a passphrase.
- **`ArtifactPayload`**: The plaintext structure serialized and then encrypted into an `EncryptedEnvelope`. Contains a map of secret key names to their plaintext values for one environment.
- **`UnsealResult`**: The return value of `unseal_artifact`. Contains a map of successfully decrypted environments (name → secret map) and a list of environment names that were skipped (inaccessible or unauthenticated).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer with the correct passphrase can round-trip (seal → unseal) any set of secrets across any number of environments with zero data loss — every key-value pair is recovered byte-for-byte.
- **SC-002**: A developer with a partial key (covering only some environments) successfully imports all accessible environments and receives a non-error notification listing the skipped environments — without the tool crashing or requiring intervention.
- **SC-003**: Any single-byte corruption of a ciphertext payload in `envy.enc` is detected within the first decryption attempt for that environment. The system adds it to the skipped list and leaves the local vault in its pre-operation state.
- **SC-004**: The `envy.enc` file produced by `seal_artifact` contains zero plaintext secret values, zero plaintext key names, and zero project identifiers when inspected with any standard text or JSON viewer.
- **SC-005**: The `envy.enc` file produces stable, deterministic output across all platforms — the same inputs always yield identically structured (though not identically encrypted) JSON, suitable for meaningful Git diffs at the environment level.

---

## Assumptions

- Passphrase strength policy (minimum length, character requirements) is enforced by the CLI layer (Phase 2 feature), not this module. This module only rejects empty/whitespace passphrases.
- Argon2id default parameters: memory 64 MiB, time cost 3 iterations, parallelism 4. These are embedded in each `EncryptedEnvelope` and may be tuned in future versions.
- The inner plaintext (secret keys + values) is serialized to JSON before encryption. This format is fixed for schema version 1.
- Path resolution for `envy.enc` (current working directory) is handled by the CLI layer. This module accepts and returns file paths as arguments.
- JSON key ordering within `SyncArtifact` is alphabetical for stable diffs; this is enforced at serialization time using a sorted map.
