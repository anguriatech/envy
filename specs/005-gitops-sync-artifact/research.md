# Research: GitOps Sync Artifact (`envy.enc`)

**Feature**: 005-gitops-sync-artifact
**Date**: 2026-03-22

---

## Decision 1: Argon2 Variant and Parameters

**Decision**: Use **Argon2id** with memory 65536 KiB (64 MiB), time cost 3, parallelism 4, output 32 bytes, random 16-byte salt per seal operation.

**Rationale**:
- Argon2id is the OWASP and IETF (RFC 9106) recommended variant for password-based key derivation: it combines Argon2i's side-channel resistance with Argon2d's GPU-hardness.
- 64 MiB memory cost makes GPU/ASIC brute-force prohibitively expensive at current hardware prices while remaining fast (<500 ms) on commodity laptops.
- Time cost 3 adds sequential computation overhead with minimal UX impact.
- 16-byte random salt (from OS CSPRNG) per-envelope prevents rainbow tables and ensures independent keys even when two environments use the same passphrase.
- The `argon2` crate (RustCrypto ecosystem, version `0.5`) provides a pure-Rust, audited implementation — mandated by the constitution.

**Alternatives considered**:
- **PBKDF2-HMAC-SHA256** (100 000 rounds): Parallelizable on GPUs — rejected. Insufficient protection against commodity hardware attacks.
- **scrypt** (N=2^17, r=8, p=1): Good parameters, but RustCrypto's `scrypt` crate is less battle-tested in production than `argon2`. Rejected for consistency with the ecosystem choice.
- **Argon2d**: Not recommended for password-based applications due to side-channel vulnerabilities in certain contexts. Rejected.

---

## Decision 2: Base64 Encoding Scheme

**Decision**: Use **Base64 standard alphabet without padding** (`base64ct`, `Encoding::Standard`) from the RustCrypto `base64ct` crate.

**Rationale**:
- `base64ct` (RustCrypto) provides constant-time encoding and decoding — preventing timing side channels during decode, which matters when the ciphertext is user-supplied.
- Standard alphabet (A–Z, a–z, 0–9, +, /) is universally supported by all JSON parsers, editors, and Git diff tools.
- No padding (`=`) avoids spurious diff noise in `envy.enc` when the ciphertext length changes.

**Alternatives considered**:
- **URL-safe Base64**: Not needed since `envy.enc` is never embedded in a URL. Rejected.
- **Hex encoding**: 2× larger output; unnecessarily bloats `envy.enc`. Rejected.
- **Standard library `base64` crate**: Not constant-time. Rejected in favour of `base64ct`.

---

## Decision 3: JSON Serialization Library and Key Ordering

**Decision**: Use **`serde_json`** for serialization. Use **`BTreeMap<String, _>`** for all map fields (environments, secret key-value pairs) to guarantee alphabetical key ordering in the output JSON.

**Rationale**:
- `serde_json` is the de-facto standard for JSON in Rust, well-audited, and already in the transitive dependency tree via other crates.
- `BTreeMap` provides ordered iteration, and `serde_json` serializes maps in insertion order — so a `BTreeMap` produces alphabetically sorted JSON keys deterministically across all platforms.
- Alphabetical ordering of environment names ensures that `git diff envy.enc` shows only the changed environment, not entire reorderings. This is a core DX requirement.

**Alternatives considered**:
- **`HashMap` with a custom serializer**: Error-prone and non-deterministic across Rust versions. Rejected.
- **`indexmap` with sorted insertion**: More complex than `BTreeMap` for no benefit here. Rejected.

---

## Decision 4: Inner Plaintext Schema (ArtifactPayload)

**Decision**: The plaintext encrypted inside each `EncryptedEnvelope` is the **JSON serialization of `BTreeMap<String, String>`** — a simple mapping of secret key names to their plaintext values. No additional wrapper or metadata is included in the inner payload.

**Rationale**:
- Minimal schema means minimal attack surface and minimal data encrypted (faster Argon2 / AES overhead).
- Alphabetical ordering of keys inside the ciphertext (via `BTreeMap`) is consistent with the outer structure.
- Including key names inside the ciphertext (not in the JSON envelope) prevents metadata leakage — an observer of `envy.enc` cannot enumerate which secrets exist in a locked environment.

**Alternatives considered**:
- **Storing key names in the envelope with values encrypted separately**: Leaks secret key names. Rejected (FR-011).
- **CBOR or MessagePack binary serialization**: Not human-debuggable during development; no meaningful size benefit for text secrets. Rejected.

---

## Decision 5: Progressive Disclosure — Wrong Key vs Tampered Payload

**Decision**: AES-GCM authentication failure is always treated as **"environment inaccessible" → graceful skip**. No attempt is made to distinguish "wrong passphrase" from "tampered ciphertext" at the cryptographic layer.

**Rationale**:
- AES-GCM produces an identical authentication failure for both wrong key and tampered ciphertext. There is no safe way to distinguish them without adding a second authentication layer, which would add complexity without a meaningful security benefit.
- The safe outcome of both failure modes is identical: the environment is not imported and the vault is not modified. A graceful skip is the correct response.
- Tampering is detectable at the file level via Git's SHA-256 commit hashes — users who need tamper evidence can verify the commit signature of `envy.enc`. This is the GitOps integrity model.
- Only **structural JSON invalidity** (malformed Base64, missing fields, unknown schema version) produces a hard `MalformedArtifact` error, because it indicates file corruption rather than a key mismatch.

**Alternatives considered**:
- **HMAC on the passphrase-derived key identifier**: Would allow distinguishing key mismatch from tamper, but reveals a passphrase fingerprint to an observer. Rejected.
- **Separate integrity hash in the envelope**: Adds redundancy over the GCM tag without benefit. Rejected.

---

## Decision 6: New Crate Dependencies

**Decision**: Add the following to `Cargo.toml`:

```toml
argon2    = "0.5"
serde_json = "1"
base64ct  = { version = "1", features = ["alloc"] }
```

**Rationale**:
- All three are from the RustCrypto ecosystem or closely associated (`serde_json` is well-audited and universally used). Constitution mandates RustCrypto for cryptographic primitives.
- These are the minimum new dependencies required; no bloat introduced.

---

## Decision 7: Module Structure

**Decision**:

- **`src/crypto/artifact.rs`** — Pure crypto primitives: `derive_key`, `encrypt_envelope`, `decrypt_envelope`. All serializable data types (`SyncArtifact`, `EncryptedEnvelope`, `KdfParams`, `ArtifactPayload`).
- **`src/core/sync.rs`** — Orchestration: `seal_artifact`, `unseal_artifact`, `write_artifact`, `read_artifact`. The `UnsealResult` type.

**Rationale**:
- Keeps the Cryptography layer (`src/crypto/`) purely about cryptographic operations and data schemas, with no file I/O or vault interaction.
- Keeps the Core layer (`src/core/`) as the orchestrator that reads from the vault, calls the crypto layer, and writes results — matching the existing pattern established by `src/core/ops.rs`.
- Strict one-way dependency: `src/core/sync.rs` imports `src/crypto/artifact.rs`, never the reverse. This satisfies Constitution Principle IV.

---

## Decision 8: Memory Safety for Decrypted Payloads

**Decision**: Decrypted `ArtifactPayload` values MUST be wrapped in `Zeroizing<_>`. The `UnsealResult.imported` map MUST use `Zeroizing<String>` for values (consistent with Phase 1's `get_env_secrets` return type).

**Rationale**:
- Constitution Principle I: in-memory representations of secrets MUST be zeroed as early as possible after use.
- The existing `decrypt` function already returns `Zeroizing<Vec<u8>>`. The artifact layer extends this contract to the deserialized payload values.
