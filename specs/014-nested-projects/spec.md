# Feature Specification: Allow Nested Envy Projects

**Feature Branch**: `014-nested-projects`
**Created**: 2026-06-10
**Status**: Draft
**Input**: User description: "Allow `envy init` in subdirectories that have a parent envy project. Today, `cmd_init` rejects initialization if ANY ancestor directory contains `envy.toml` (via `find_manifest` walking upward). This blocks a legitimate monorepo / multi-project use case."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Initialise a nested project (Priority: P1)

A developer working in a monorepo wants to store org-wide credentials at the root level and project-specific credentials in each project subdirectory. The monorepo layout is:

```
/monorepo/
  envy.toml     ← org-wide credentials (e.g., CI tokens)
  envy.enc
  /project-a/
    envy init   ← should succeed — project-specific credentials
    envy.toml   ← different UUID from parent
    envy.enc
  /project-b/
    envy init   ← should succeed — different UUID again
    envy.toml
    envy.enc
```

Today, running `envy init` in `project-a/` fails with "parent project detected" because the walker finds the root's `envy.toml`. The developer must choose an arbitrary directory outside the monorepo to init, defeating the point of co-locating secrets with their project.

**Why this priority**: This is the sole purpose of the spec — removing the ParentProjectExists restriction. Without this, all other scenarios are blocked.

**Independent Test**: Can be tested end-to-end by creating a parent project, then running `envy init` in a subdirectory. The child's `envy.toml` must contain a different UUID than the parent's. Subsequent operations from the child's directory must use the child's vault entry.

**Acceptance Scenarios**:

1. **Given** `/parent/` already has a valid `envy.toml`, **When** the user runs `envy init` in `/parent/child/`, **Then** the CLI exits 0 and creates a new `envy.toml` in `/parent/child/` with a UUID that differs from the parent's `envy.toml`.
2. **Given** a parent `envy.toml` in `/grandparent/` and no `envy.toml` in `/grandparent/parent/`, **When** the user runs `envy init` in `/grandparent/parent/child/`, **Then** the CLI exits 0 and creates a new `envy.toml` in the cwd (the closest ancestor walker finds the grandparent, but the cwd has no manifest → proceed).
3. **Given** a nested project has been initialised, **When** the user runs `envy set KEY=VALUE` from the child's directory, **Then** the secret is stored under the child's UUID in the vault, NOT under the parent's UUID.

---

### User Story 2 - `AlreadyInitialised` still rejects init in an already-initialised directory (Priority: P1)

A developer accidentally runs `envy init` in a directory that already has `envy.toml`. The CLI must still reject this as "already initialised" — the removal of the ParentProjectExists check must not regress into allowing double-init.

**Why this priority**: This is the safety rail — the old behaviour must be preserved for the exact cwd match. Without this, a user could silently re-initialise and lose their project UUID reference.

**Independent Test**: Can be tested by running `envy init` in a directory that already has `envy.toml` and asserting the CLI returns `AlreadyInitialised` error.

**Acceptance Scenarios**:

1. **Given** an `envy.toml` exists in `/parent/child/`, **When** the user runs `envy init` in `/parent/child/`, **Then** the CLI returns `AlreadyInitialised` (exit 3) and does not overwrite the existing `envy.toml`.

---

### User Story 3 - A parent project can be deleted without affecting the child (Priority: P2)

An org restructures its monorepo and the root-level `envy.toml` is removed. The child project in `/parent/child/` must continue to work — its own `envy.toml` is still present and is found first by `find_manifest`.

**Why this priority**: This is a resilience test. It is not a new feature — it is verified as a natural consequence of the closest-ancestor resolution. The test confirms the property but no new code is needed.

**Independent Test**: Initialise a parent + child, then delete the parent's `envy.toml`. Run `envy list` from the child's directory — it must exit 0 and show the child's secrets.

**Acceptance Scenarios**:

1. **Given** a parent `envy.toml` in `/parent/` and a child `envy.toml` in `/parent/child/`, **When** the parent's `envy.toml` is deleted and the user runs `envy list` from `/parent/child/`, **Then** the CLI exits 0 and shows the child's secrets (not an error about a missing manifest, because the child's `envy.toml` is found first).

---

### Edge Cases

- **What happens when both parent and child use the same environment name (e.g., "development")?**
  Each project has its own UUID in the vault, so the environments are stored in separate rows. No collision. The vault's `projects` table separates them by UUID, and the `environments` table has a foreign key to `projects`.

- **What happens when the child's `envy.toml` is deleted?**
  `find_manifest` walks upward and finds the parent's `envy.toml`. The user is now operating with the parent's secrets. This is the correct fallback — no data loss occurred (the child's secrets are still in the vault under the child's UUID).

- **What happens when there is a chain of 3+ nested projects?**
  `find_manifest` always returns the closest ancestor with an `envy.toml`. The CLI operates with that project's UUID. If the user wants a different ancestor, they must `cd` to that directory.

- **What happens when the user runs `envy encrypt` from a nested project?**
  The artifact (`envy.enc`) is written next to the closest `envy.toml` (the child's). The parent's artifact is untouched. Each project manages its own artifact independently.

- **What happens when the parent's `envy.enc` and the child's `envy.enc` coexist?**
  They are two independent artifacts with different UUIDs. The CLI resolves only one (whichever `envy.toml` `find_manifest` returns). No cross-contamination occurs.

- **What happens when the global vault (`~/.envy/vault.db`) already has data for the parent's UUID and the child's UUID?**
  No conflict. The vault stores projects, environments, and secrets per UUID. Both UUIDs coexist peacefully in the same vault file.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `envy init` command MUST succeed in a directory whose ancestors contain an `envy.toml`, as long as the cwd itself does NOT contain an `envy.toml`.
- **FR-002**: The `envy init` command MUST still return the `AlreadyInitialised` error when the cwd itself contains an `envy.toml`.
- **FR-003**: When a nested project is initialised, the new `envy.toml` MUST contain a UUID that is different from the parent's `envy.toml` (i.e., each project gets its own vault identity).
- **FR-004**: The `find_manifest` resolution MUST continue to return the closest ancestor `envy.toml` when looking up a project context for any operation (unchanged behaviour).
- **FR-005**: Subsequent operations from a nested project directory (e.g., `envy set`, `envy list`, `envy encrypt`) MUST use the nested project's UUID, not the parent's.
- **FR-006**: The `ParentProjectExists` error variant MUST no longer be returned by `cmd_init`. It SHOULD be either removed from the `CliError` enum or marked as deprecated and mapped to an exit code 3 (init conflict) if an external caller matches on it.
- **FR-007**: The vault structure MUST not be changed. The shared vault already supports multiple projects via UUID differentiation.

### Key Entities *(include if feature involves data)*

- **Project UUID**: a unique v4 UUID stored in `envy.toml`'s `project_id` field. Every directory with an `envy.toml` has its own UUID. The vault stores secrets keyed by (UUID, environment name, secret key). Nested projects are simply two different UUIDs in the same vault file.
- **Manifest resolution**: the `find_manifest` walker from `src/core/manifest.rs` walks upward from the cwd, returning the first `envy.toml` found. For nested projects, this means the child's manifest takes precedence when running commands from the child's directory. The parent manifest is only resolved when no closer manifest exists.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can run `envy init` in a subdirectory of an existing envy project, and the command exits 0 with a success message showing the new project UUID. The parent's `envy.toml` is untouched.
- **SC-002**: The new `envy.toml` created in the subdirectory contains a `project_id` that is different from the parent's `project_id` (verified by comparing the UUID field in both TOML files).
- **SC-003**: Running `envy init` in a directory that already has `envy.toml` still exits with a non-zero code and does not overwrite the file (regression test for `AlreadyInitialised`).
- **SC-004**: After initialising a nested project, running `envy set KEY=VALUE` from the child's directory stores the secret under the child's UUID, and running `envy list` from the child's directory shows only the child's secrets (verified by listing from both the parent and child directories and confirming no overlap).
- **SC-005**: The full test suite (`cargo test`) passes without modification to pre-existing tests. The `find_manifest_in_parent_dir` test in `src/core/manifest.rs` continues to pass — it already verifies the correct walker behaviour for ancestors.
- **SC-006**: `cargo clippy -- -D warnings` passes. No dead-code warnings from `ParentProjectExists` if it is removed, or a `#[deprecated]` annotation if it is kept.

## Out of Scope *(deferred — documented as follow-ups)*

The following items are explicitly out of scope for this spec and are documented here so that future specs can pick them up:

- **Changing the vault structure**: no `path` column or parent-child relationship is added to the projects table. The vault remains flat (one row per UUID).
- **Changing `envy.toml` design**: no new fields (e.g., `parent_project_id`) are added.
- **Recursive operations**: no `envy list --all` or `envy encrypt --recursive` that aggregates secrets from all ancestor projects.
- **Improving `ManifestNotFound` error messages**: the existing error message is unchanged.
- **Any UI change beyond `cmd_init`**: no new flags, no new subcommands, no changes to any other command.

## Versioning

- Bump `Cargo.toml` from `0.3.1` to `0.3.2` (patch). Rationale: this is a behaviour relaxation that does not break existing workflows — users who already have a single `envy.toml` per tree see no change. Users who were previously blocked by `ParentProjectExists` can now init. Existing projects continue to work identically.
