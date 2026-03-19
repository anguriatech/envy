# Feature Specification: Core Logic

**Feature Branch**: `003-core-logic`
**Created**: 2026-03-19
**Status**: Draft

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Project Context Auto-Resolved (Priority: P1)

When a developer runs any Envy command from within a project directory (or any of its
subdirectories), the tool automatically locates the `envy.toml` manifest file, reads the
`project_id`, and knows which project to operate on — without the developer having to
specify a project name or ID on every command.

**Why this priority**: All secret operations require a `project_id`. Without context
resolution, every command would need an explicit `--project` argument, making the tool
unusable. This is the prerequisite for every other user story in this feature.

**Independent Test**: From a directory containing `envy.toml`, request the current
context. Verify the returned `project_id` matches the one in the file. From a
subdirectory, verify the same result. From a directory with no `envy.toml` in any parent,
verify a clear "not initialized" error is returned.

**Acceptance Scenarios**:

1. **Given** the current directory contains `envy.toml` with a valid `project_id`, **When**
   the Core layer resolves context, **Then** it returns that `project_id` successfully.
2. **Given** the current directory has no `envy.toml` but a parent directory does, **When**
   the Core layer resolves context, **Then** it finds and reads the parent's `envy.toml`.
3. **Given** no `envy.toml` exists in the current directory or any parent up to the
   filesystem root, **When** the Core layer resolves context, **Then** it returns a
   `ManifestNotFound` error with an actionable message (e.g., "run envy init first").
4. **Given** an `envy.toml` exists but is malformed or missing the `project_id` field,
   **When** the Core layer resolves context, **Then** it returns a `ManifestInvalid` error.

---

### User Story 2 — Secret Stored and Retrieved Securely (Priority: P1)

When a developer sets a secret (`DATABASE_URL=postgres://...`), the Core layer encrypts
the plaintext value using the master key before passing the ciphertext to the Database
layer. The plaintext never reaches the database. When the developer retrieves it, the Core
layer fetches the ciphertext and decrypts it, returning the plaintext in memory. When the
environment where a non-existent secret is requested, a clear "not found" error is
returned.

**Why this priority**: This is the fundamental value proposition of Envy — secrets stored
encrypted, retrieved decrypted. Without this, the tool cannot replace `.env` files.

**Independent Test**: Call `set_secret` with a known plaintext. Inspect the `secrets`
table — `value_encrypted` must differ from the plaintext. Call `get_secret` and assert the
decrypted output matches the original plaintext exactly. Call `get_secret` for a
non-existent key and assert the appropriate "not found" error.

**Acceptance Scenarios**:

1. **Given** a valid project and environment, **When** `set_secret("DB", "postgres://...")`
   is called, **Then** the secret is stored and retrievable; the vault's raw bytes do not
   contain the plaintext string.
2. **Given** a stored secret, **When** `get_secret("DB")` is called with the correct
   master key, **Then** the exact original plaintext is returned, zeroed on drop.
3. **Given** an existing secret, **When** `set_secret` is called again with the same key
   and a new value, **Then** the stored value is updated (upsert semantics); the old
   ciphertext is replaced.
4. **Given** a valid environment, **When** `get_secret` is called for a key that does not
   exist, **Then** a clear "not found" error is returned.
5. **Given** a valid environment with multiple secrets, **When** `list_secret_keys` is
   called, **Then** only the key names are returned (no decryption, no plaintext values
   exposed), ordered alphabetically.
6. **Given** an existing secret, **When** `delete_secret` is called, **Then** the secret
   is permanently removed from the vault.

---

### User Story 3 — All Secrets Prepped for Process Injection (Priority: P1)

When a developer runs `envy run -- <command>`, the Core layer fetches and decrypts every
secret in the current project/environment and returns them as a map of
`key → plaintext_value`. The CLI layer uses this map to set environment variables for the
child process. All plaintext values are automatically zeroed from memory when the map is
dropped.

**Why this priority**: The `run` command is the core DX feature of Envy — it replaces
sourcing a `.env` file. Without this, the tool has no practical advantage over a static
file.

**Independent Test**: Store three secrets for an environment. Call `get_env_secrets`.
Assert the returned map contains all three keys with their correct decrypted values. Drop
the map and verify memory zeroing behaviour.

**Acceptance Scenarios**:

1. **Given** an environment with three secrets, **When** `get_env_secrets` is called,
   **Then** a map with exactly three entries is returned, each value decrypted correctly.
2. **Given** an environment with no secrets, **When** `get_env_secrets` is called, **Then**
   an empty map is returned (not an error).
3. **Given** one secret whose ciphertext is corrupted, **When** `get_env_secrets` is
   called, **Then** the operation fails with a decryption error — no partial map is
   returned.

---

### User Story 4 — Environment Defaulting and Auto-Creation (Priority: P2)

When a developer runs `envy set API_KEY=abc` without specifying `--env`, the Core layer
defaults to the `development` environment. If that environment does not yet exist in the
vault, it is created automatically. Explicitly specified environments are used as-is (after
lowercasing validation).

**Why this priority**: Eliminates friction for the common case. A developer should be able
to run `envy set` immediately after `envy init` without first running `envy env create`.

**Independent Test**: On a freshly initialized project (no environments yet), call
`set_secret` without specifying an environment. Verify that the `development` environment
is auto-created and the secret is stored in it.

**Acceptance Scenarios**:

1. **Given** a project with no environments, **When** `set_secret` is called without an
   explicit environment name, **Then** the `development` environment is auto-created and
   the secret is stored there.
2. **Given** the `development` environment already exists, **When** `set_secret` is called
   without an explicit environment name, **Then** the existing environment is used (no
   duplicate is created).
3. **Given** an explicit `--env staging` flag, **When** any secret operation is called,
   **Then** the `staging` environment is used, creating it if it does not exist.

---

### Edge Cases

- What if the `project_id` in `envy.toml` does not exist in the vault (vault was reset)?
  → `DbError::NotFound` surfaces as a clear "project not found" error with a recovery hint.
- What if the master key was rotated (corrupted ciphertext)?
  → `CryptoError::DecryptionFailed` surfaces as a "vault key mismatch" error.
- What if the secret key name is empty or contains `=` or null bytes?
  → Rejected with an `InvalidSecretKey` error before any DB or Crypto call is made.
- What if two concurrent processes call `set_secret` with the same key?
  → The DB layer's `INSERT OR REPLACE` guarantees atomicity; the last write wins.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Core layer MUST resolve the current project context by walking up the
  directory tree from the current working directory to find the nearest `envy.toml`.
- **FR-002**: The Core layer MUST expose a function to create an `envy.toml` manifest in a
  given directory, writing the `project_id` field.
- **FR-003**: `set_secret` MUST encrypt the plaintext value via the Crypto layer before
  passing it to the Database layer; the plaintext MUST NOT be logged or passed to the DB.
- **FR-004**: `get_secret` MUST fetch the ciphertext and nonce from the Database layer and
  decrypt via the Crypto layer; the returned plaintext MUST be wrapped in a zeroing
  container.
- **FR-005**: `list_secret_keys` MUST return only key names — no ciphertext, nonce, or
  decrypted values.
- **FR-006**: `delete_secret` MUST propagate `NotFound` if the key does not exist.
- **FR-007**: `get_env_secrets` MUST decrypt all secrets for an environment and return them
  as a map of `String → zeroing-string`. If any single decryption fails, the entire
  operation MUST fail — no partial maps.
- **FR-008**: All secret operations MUST accept an optional environment name; when omitted
  or empty, `"development"` MUST be used as the default.
- **FR-009**: When the target environment does not exist, `set_secret` and `get_env_secrets`
  MUST auto-create it (for `set_secret`) or return an appropriate error (for read-only
  operations on non-existent environments).
- **FR-010**: Secret key names MUST be validated before any operation: non-empty, no `=`
  character, valid UTF-8. Violations MUST return `InvalidSecretKey`.
- **FR-011**: The Core layer MUST expose a typed `CoreError` enum that wraps `DbError` and
  `CryptoError` and adds manifest-specific variants.
- **FR-012**: The Core layer MUST NOT import from the UI/CLI layer (Constitution Principle IV).
- **FR-013**: The Core layer MUST NOT log or surface plaintext secret values or the master
  key in error messages (Constitution Principle I).

### Key Entities

- **Manifest** (`envy.toml`): A TOML file in the project root containing the `project_id`
  UUID that links the local directory to its vault entry. Created by `envy init`, read by
  all other commands.
- **Context**: The resolved runtime state — `project_id` (from manifest) + active
  environment name (from flag or default `"development"`). Passed to all secret operations.
- **Secret Key**: A UTF-8 string name for a secret (e.g., `DATABASE_URL`). Must not be
  empty or contain `=`.
- **Secret Value**: The plaintext secret string. Encrypted by the Core layer before
  storage; decrypted and returned zeroed after retrieval.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Context resolution (finding and parsing `envy.toml`) completes in under
  50 milliseconds on any standard filesystem.
- **SC-002**: `set_secret` + `get_secret` round-trip completes in under 5 milliseconds
  (excluding first-run key generation).
- **SC-003**: `get_env_secrets` with 100 secrets completes in under 100 milliseconds
  (decryption overhead is imperceptible to the user).
- **SC-004**: 100% of `get_env_secrets` calls either return all secrets or return an error —
  no partial maps are ever returned.
- **SC-005**: The Core layer passes `cargo clippy -- -D warnings` with zero warnings.
- **SC-006**: All Core layer functions have unit or integration tests covering primary paths
  and all error variants defined in `CoreError`.

### Assumptions

- `envy.toml` is a simple TOML file with at minimum a `project_id` string field. Other
  fields may be added in future features without breaking this contract.
- The Core layer does not open or close the vault — the CLI layer is responsible for
  calling `Vault::open` and `Vault::close`. Core operations receive an already-open `Vault`
  reference.
- The master key lifetime is managed by the CLI layer; Core operations receive a `&[u8; 32]`
  reference. The Core layer MUST NOT clone or persist the key.
- Environment name comparison is case-sensitive; callers MUST pass lowercase names. The
  Core layer normalizes to lowercase as a defense-in-depth measure.
- Secret key validation rejects empty strings and strings containing `=` but does not
  impose any other format restriction (uppercase, underscores, etc. are all valid).
