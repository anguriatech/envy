# Feature Specification: Vault Core Data Model

**Feature Branch**: `001-vault-db-schema`
**Created**: 2026-03-18
**Status**: Draft
**Input**: Phase 1 MVP — Core Database Schema for Envy Vault

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Register a New Project in the Vault (Priority: P1)

A developer runs `envy init` in their project directory. The vault must be able to store a
unique record for this project so that future commands know which vault entry to read from
and write to. The developer does not interact with the vault directly; the CLI resolves it
automatically from then on.

**Why this priority**: Without the ability to register and identify a project, every other
feature (storing secrets, reading them at runtime) is impossible. This is the root entity
of the entire data model.

**Independent Test**: Run `envy init` in an empty directory. Verify the vault contains a
new project record with a unique identifier. Running `envy init` again in the same
directory MUST NOT create a duplicate record.

**Acceptance Scenarios**:

1. **Given** no prior project record exists for the current directory, **When** the developer
   runs `envy init`, **Then** the vault contains exactly one new project record with a unique
   identifier and creation timestamp.
2. **Given** a project record already exists for the current directory, **When** the developer
   runs `envy init` again, **Then** the vault record is unchanged and no duplicate is created.
3. **Given** a project record exists, **When** any subsequent command references it,
   **Then** the record can be resolved deterministically by its unique identifier.

---

### User Story 2 - Segregate Secrets by Environment (Priority: P1)

A developer manages secrets for multiple deployment contexts (e.g., local development,
staging, production) under the same project. The vault must allow storing and retrieving
secrets independently per environment so that production credentials are never accidentally
injected into a development process.

**Why this priority**: Multi-environment support is a core Phase 1 deliverable. Without it,
Envy cannot replace `.env` files which developers already maintain per-environment manually.

**Independent Test**: Store a different value for the same secret key in two environments
under the same project. Retrieve each — verify the correct value is returned for each
environment, and the environments are fully isolated.

**Acceptance Scenarios**:

1. **Given** a project exists, **When** the developer stores a secret in the `development`
   environment, **Then** the secret is only visible when querying the `development` environment.
2. **Given** the same secret key exists in both `development` and `production`, **When** the
   developer retrieves it for `production`, **Then** the `production` value is returned, not
   the `development` value.
3. **Given** no environment is specified by the developer, **When** a secret is stored or
   retrieved, **Then** the `development` environment is used as the default.

---

### User Story 3 - Store and Retrieve Individual Secrets (Priority: P1)

A developer stores a named secret value (e.g., `DATABASE_URL`, `STRIPE_KEY`) under a
specific project and environment. Later, the same developer (or an automated process) must
be able to retrieve the exact value. At no point during storage or retrieval MUST the value
be readable by inspecting the vault file directly.

**Why this priority**: This is the atomic unit of value Envy provides. All other features
(the `run` wrapper, export, CI/CD) depend on this working correctly.

**Independent Test**: Store a secret value. Open the vault file with any text editor or
hex viewer — verify the value is not readable in plaintext. Then retrieve the value via
`envy get` — verify it matches what was stored exactly, character for character.

**Acceptance Scenarios**:

1. **Given** a project and environment exist, **When** a secret key-value pair is stored,
   **Then** the vault contains exactly one record for that key in that environment.
2. **Given** a secret already exists for a key, **When** a new value is stored for the same
   key in the same environment, **Then** the old value is replaced and last-modified time is
   refreshed.
3. **Given** the vault file is inspected directly (e.g., opened as a binary file),
   **Then** no secret value is readable as plaintext under any circumstances.
4. **Given** a secret is stored, **When** it is retrieved via the CLI, **Then** the returned
   value matches the originally stored value with byte-for-byte accuracy.

---

### Edge Cases

- What happens when two processes write to the vault simultaneously? The storage layer MUST
  serialize concurrent writes without data corruption (ACID guarantee required).
- What happens when the vault file is deleted mid-session? The vault MUST be re-creatable
  from scratch without leaving any orphaned state; the OS Credential Manager key remains
  valid.
- What happens if a secret key contains special characters (spaces, `=`, unicode)? The
  storage system MUST accept and return any valid UTF-8 string as a key or value.
- What happens when a secret is deleted? The record MUST be fully removed from the vault
  with no recoverable remnant — no soft-delete in Phase 1.
- What happens when the same environment name is provided in different cases (e.g.,
  `Development` vs `development`)? Environment names MUST be normalized to lowercase to
  prevent accidental duplication.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The vault MUST uniquely identify each registered project by a globally unique
  identifier that is stable across machines (suitable for future syncing).
- **FR-002**: The vault MUST support multiple named environments per project, with
  `development` as the default when none is specified.
- **FR-003**: The vault MUST store one secret value per key per environment per project;
  storing a key that already exists MUST overwrite the previous value.
- **FR-004**: The vault MUST record the creation time and last-modification time for every
  project, environment, and secret record.
- **FR-005**: No secret value MUST ever be stored in a form that is directly readable from
  the vault file without the master key.
- **FR-006**: The vault MUST enforce referential integrity: an environment MUST belong to
  a valid project, and a secret MUST belong to a valid environment.
- **FR-007**: Deleting a project MUST cascade to delete all its environments and secrets,
  leaving no orphaned records.
- **FR-008**: Deleting an environment MUST cascade to delete all its secrets.
- **FR-009**: The vault schema MUST be initialized automatically on first use; the
  developer MUST NOT need to run any manual migration step.
- **FR-010**: The vault MUST be structured to accommodate future additions (audit log
  entries, user/team records, permission assignments) without requiring breaking changes to
  the existing tables.

### Key Entities

- **Project**: Represents one developer project linked to a directory on disk. Has a unique
  identifier and a human-readable name. Is the root entity; all other data is scoped to it.
- **Environment**: A named deployment context within a project (e.g., `development`,
  `staging`, `production`). Scopes secrets so they remain isolated from each other.
- **Secret**: A named key-value pair belonging to one environment. The value is sensitive
  and MUST be protected at rest. Tracks when it was created and last changed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can initialize a project and store their first secret in under
  10 seconds total elapsed time on any supported OS.
- **SC-002**: Retrieving a secret value returns the exact stored value with 100% fidelity
  across 1,000 consecutive store-retrieve round-trips with no corruption.
- **SC-003**: Inspecting the vault file directly (binary or text view) reveals zero
  plaintext secret values across a test set of 100 stored secrets.
- **SC-004**: The schema supports at least 10,000 secrets across 100 projects and 10
  environments per project without query degradation on a standard developer laptop.
- **SC-005**: All referential integrity constraints (cascade deletes, foreign keys) are
  enforced 100% of the time — no orphaned records exist after any delete operation.
- **SC-006**: The schema requires zero manual migration steps from a developer when the
  vault is first created on a new machine.

## Assumptions

- The vault is a single-user, single-machine store in Phase 1. Multi-user sync is deferred
  to Phase 2; the UUID-based schema is designed to support it without breaking changes.
- Environment names are short ASCII/UTF-8 labels. No maximum length is enforced in Phase 1
  beyond what the storage system naturally supports.
- Secret keys follow the convention of environment variable names (uppercase, underscores)
  but the schema MUST NOT enforce this constraint — validation is the CLI's responsibility.
- The master encryption key is managed externally (OS Credential Manager) and is not
  stored in or derived from the schema itself.
- Soft-delete (tombstoning) is explicitly out of scope for Phase 1. If needed for audit
  logs in Phase 3, it will be added as a separate table without altering existing ones.
