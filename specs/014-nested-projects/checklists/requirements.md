# Specification Quality Checklist: Allow Nested Envy Projects

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-10
**Feature**: [spec.md](spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded (Out of Scope section)
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Validation Notes

### Content Quality
- The spec describes the change in terms of user-visible behaviour: "envy init succeeds in a subdirectory", "envy.toml gets a different UUID", "AlreadyInitialised still rejects double-init". It does not mention Rust, clap, dialoguer, `find_manifest`, `create_manifest`, or `ParentProjectExists` as implementation details (the Key Entities section names the entities being changed as a reference for the planner, not as a requirement for the reader).
- Acceptance scenarios are written in business language (e.g., "A developer working in a monorepo wants different credentials per project").
- All mandatory sections (User Scenarios & Testing, Requirements, Success Criteria) are present and non-empty.

### Requirement Completeness
- No `[NEEDS CLARIFICATION]` markers were used. The user description was very detailed.
- Every FR is testable. FR-001 (init in subdirectory) is testable by creating a parent project and running init in a subdirectory — this is SC-001.
- Success criteria are measurable: UUID comparison (SC-002), exit codes (SC-001, SC-003), secret isolation (SC-004), test suite pass count (SC-005), clippy pass (SC-006).
- Edge cases include: same env name across projects (no collision), child manifest deleted (fallback to parent), 3+ nesting levels, encrypt from nested project, coexisting artifacts.
- Scope is explicitly bounded: no vault changes, no `envy.toml` design changes, no recursive operations.

### Feature Readiness
- Three user stories cover the primary flows: US1 (P1, the core relaxation), US2 (P1, the regression test), US3 (P2, the resilience test).
- Each FR maps to at least one acceptance scenario. FR-001/FR-002 map to US1/US2 scenarios, FR-003 maps to SC-002, FR-005 maps to SC-004.
- The Out of Scope section lists 5 items that were deliberately deferred.

### Assumptions Made
1. The `ParentProjectExists` error variant is kept in the error enum for backward compatibility (it may be matched by external code) but is mapped to exit code 3 (init conflict). It is no longer returned by `cmd_init`. This is a reasonable default given the user says "remove or deprecate" — the safer approach is to keep it but document it as deprecated.
2. No new tests exist for `cmd_init` in `src/cli/commands.rs` test module today — the spec notes that the tests are NEW, not updates to existing tests. The `find_manifest_in_parent_dir` test in `src/core/manifest.rs` is pre-existing and already verifies the correct walker behaviour.
3. The version bump from 0.3.1 to 0.3.2 is a patch per the project's pre-1.0 versioning convention (behaviour relaxation, no breaking change).
4. The vault path `~/.envy/vault.db` is unchanged — nested projects coexist in the same vault file, differentiated by UUID. This is already true today (multiple non-nested projects on the same machine already share the vault via different UUIDs).

### Items NOT Requiring Follow-up
No clarifications were needed; no items are flagged as incomplete. The spec is ready for the planning phase.
