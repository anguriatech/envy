# Feature Specification: Crypto Layer

**Feature Branch**: `002-crypto-layer`
**Created**: 2026-03-19
**Status**: Draft

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Vault Unlocks on First Run (Priority: P1)

When a developer runs any Envy command for the first time on a machine, the tool must
automatically generate a 32-byte master key, store it securely in the OS Credential
Manager, and confirm the vault is ready — all without exposing the key in any log, file,
or environment variable.

**Why this priority**: Every other feature depends on the master key being available. If
this flow fails, the vault cannot open and nothing else works.

**Independent Test**: On a machine with no prior Envy key, request the master key. Verify
the OS Credential Manager contains a new entry and the returned key is exactly 32 bytes.

**Acceptance Scenarios**:

1. **Given** no Envy key exists in the OS Credential Manager, **When** the crypto layer is
   asked for the master key, **Then** it generates a cryptographically random 32-byte key,
   stores it in the OS Credential Manager, and returns it.
2. **Given** a key already exists, **When** the crypto layer is asked again, **Then** it
   returns the same key without generating a new one.
3. **Given** the OS Credential Manager is unavailable or access is denied, **When** a key
   is requested, **Then** the operation fails with a `KeyringUnavailable` error — never
   silently falling back to a plaintext file.

---

### User Story 2 — Secret Encrypted Before DB Write (Priority: P1)

When the Core layer calls the crypto layer to encrypt a plaintext secret before storing it,
the layer returns a ciphertext blob and a fresh 12-byte nonce. The plaintext never touches
disk. When the Core layer retrieves a secret, it passes the stored ciphertext and nonce to
the crypto layer and receives the original plaintext back.

**Why this priority**: This is the defense-in-depth requirement from the constitution.
Even if the SQLCipher file-level encryption were somehow bypassed, individual secret values
must remain ciphertext blobs.

**Independent Test**: Call `encrypt` with a known plaintext and the master key. Verify the
returned ciphertext does not contain the plaintext in readable form. Call `decrypt` with
the same key, ciphertext, and nonce — confirm the original plaintext is recovered exactly.

**Acceptance Scenarios**:

1. **Given** a plaintext secret and the master key, **When** `encrypt` is called, **Then**
   it returns a ciphertext blob and a 12-byte nonce, neither of which contains the original
   plaintext in readable form.
2. **Given** the ciphertext, nonce, and the correct master key, **When** `decrypt` is
   called, **Then** the original plaintext is recovered exactly.
3. **Given** the ciphertext, nonce, and a wrong key, **When** `decrypt` is called,
   **Then** it returns a `DecryptionFailed` error — never a garbled plaintext.
4. **Given** a ciphertext with any byte tampered, **When** `decrypt` is called, **Then**
   the AEAD authentication tag check fails and a `DecryptionFailed` error is returned
   (integrity guarantee).

---

### User Story 3 — Sensitive Memory Zeroed After Use (Priority: P2)

When the master key or plaintext secret is no longer needed in memory, its bytes are zeroed
before the allocation is released, preventing residual secrets from lingering in heap memory
or appearing in core dumps.

**Why this priority**: Derived from Constitution Principle I. Invisible to the user but
mandatory for security compliance.

**Independent Test**: After a key or plaintext container is dropped, the backing bytes at
the same memory address read as zeros (verifiable in unit tests using controlled
allocations).

**Acceptance Scenarios**:

1. **Given** a master key held in a secure container, **When** the container is dropped,
   **Then** the bytes at its memory location are zeroed before deallocation.
2. **Given** a decrypted plaintext value held in a secure container, **When** the caller
   drops it, **Then** its bytes are zeroed before deallocation.

---

### Edge Cases

- What happens when the OS Credential Manager returns a key shorter or longer than 32
  bytes (e.g., corrupted store)? → Validate length on retrieval; return `KeyCorrupted`.
- What happens when `encrypt` is called with an empty plaintext? → Must succeed; empty
  plaintext is valid AEAD input and produces valid authenticated ciphertext.
- What happens when `decrypt` receives a nonce that is not exactly 12 bytes? → Return
  `InvalidNonce` before attempting decryption.
- What happens when two calls to `encrypt` use the same plaintext? → Each call generates a
  fresh random nonce, so ciphertexts will differ (nonce reuse is structurally impossible).

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The crypto layer MUST retrieve the master key exclusively from the OS
  Credential Manager; it MUST NOT accept keys from environment variables, config files,
  or command-line arguments.
- **FR-002**: The crypto layer MUST generate a cryptographically random 32-byte master key
  when none exists, store it in the OS Credential Manager, and return it — atomically from
  the caller's perspective.
- **FR-003**: The master key MUST be validated as exactly 32 bytes upon retrieval; any
  other length MUST be rejected with a `KeyCorrupted` error.
- **FR-004**: The encrypt function MUST generate a fresh, cryptographically random 12-byte
  nonce for every call; reusing nonces across calls MUST be structurally impossible.
- **FR-005**: The encrypt function MUST return both the ciphertext blob and the 12-byte
  nonce as separate values for the caller to store in the DB schema columns
  `value_encrypted` and `value_nonce`.
- **FR-006**: The decrypt function MUST verify the AEAD authentication tag before returning
  any bytes; a tag mismatch MUST return `DecryptionFailed` without exposing partial
  plaintext.
- **FR-007**: The decrypt function MUST reject nonces that are not exactly 12 bytes with an
  `InvalidNonce` error.
- **FR-008**: All types that hold key material or plaintext secrets MUST zero their backing
  memory when dropped.
- **FR-009**: The crypto layer MUST expose a typed `CryptoError` enum covering at minimum:
  `KeyNotFound`, `KeyCorrupted`, `KeyringUnavailable`, `EncryptionFailed`,
  `DecryptionFailed`, `InvalidNonce`.
- **FR-010**: The crypto layer MUST NOT import from the UI/CLI or Core layers (Constitution
  Principle IV).
- **FR-011**: The crypto layer MUST NOT log or surface plaintext key material or secret
  values in error messages or diagnostics (Constitution Principle I).

### Key Entities

- **Master Key**: A 32-byte cryptographically random value. Stored exclusively in the OS
  Credential Manager under a fixed service name and account. Never written to disk.
- **Ciphertext Blob** (`value_encrypted`): Output of AEAD encryption. Opaque byte
  sequence; meaningful only when paired with its nonce. Stored in the `secrets` table.
- **Nonce** (`value_nonce`): A 12-byte (96-bit) random value, unique per encryption call.
  Must be stored alongside the ciphertext. Stored in the `secrets` table.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Encrypting and decrypting a 1 KB secret completes in under 1 millisecond on
  any modern consumer hardware (encryption overhead is imperceptible to the user).
- **SC-002**: 100% of `encrypt` calls produce a different ciphertext for the same plaintext
  input (nonce uniqueness enforced by construction, verified in tests).
- **SC-003**: Decryption with a wrong key or tampered ciphertext always returns an error —
  zero instances of partial or incorrect plaintext returned.
- **SC-004**: All types carrying key material pass memory-zeroing tests: backing bytes read
  as zeros after the value is dropped.
- **SC-005**: The crypto layer passes `cargo clippy -- -D warnings` and `cargo audit` with
  zero warnings or vulnerabilities attributable to this feature.
- **SC-006**: The OS Credential Manager path (store, retrieve, auto-generate) works
  end-to-end on Linux (Secret Service / libsecret) without requiring user configuration.

### Assumptions

- The caller (Core layer) is responsible for passing the correct nonce when decrypting; the
  crypto layer does not manage nonce-to-ciphertext associations.
- The keyring service name (`envy`) and account name (`master-key`) are fixed constants
  defined in this layer; they are not configurable by the caller.
- Key rotation (replacing the master key and re-encrypting all secrets) is out of scope for
  this feature but the API must not obstruct a future rotation feature.
- The `zeroize` crate is used for memory zeroing; `Zeroizing<T>` wrappers are preferred
  over manual zeroing.
