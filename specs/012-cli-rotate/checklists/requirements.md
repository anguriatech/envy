# Specification Quality Checklist: Envelope Passphrase Rotation

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
- The spec describes the `rotate` command in terms of user-visible behaviour (prompts, error messages, exit codes, success messages). It does not mention Rust, crates, modules, AES-GCM, SQLCipher, dialoguer, or any other implementation detail.
- Acceptance scenarios are written in business language (e.g. "A tech lead rotates the production envelope"), not in technical terms.
- All mandatory sections (User Scenarios & Testing, Requirements, Success Criteria) are present and non-empty.

### Requirement Completeness
- No `[NEEDS CLARIFICATION]` markers were used. The user description was very detailed and left no critical ambiguities. Three "informed-guess" decisions were made and documented in the spec:
  1. The exact error message text (derived from the user's prose).
  2. The "forward-only" confirmation message wording (derived from the user's prose).
  3. The exact env-var name suffix `_NEW` (derived from the user's prose: `ENVY_PASSPHRASE_<ENV>_NEW`).
- Every FR is testable. For example, FR-005 (verify the current passphrase before any change) is testable by attempting a wrong current passphrase and verifying the artifact is unchanged (which is the SC-002 success criterion).
- Success criteria are measurable: SHA-256 of artifact before/after (SC-002, SC-007), exit code (SC-005), count comparison (SC-003), wall-clock time (SC-001).
- Edge cases include: new = current passphrase, confirmation mismatch, whitespace-only passphrase, missing artifact, missing envelope in artifact, partial-write crash, Ctrl-C after write.

### Feature Readiness
- The five user stories cover the primary flows (P1 single-env, P1 wrong-passphrase safety, P2 headless, P2 multi-env, P3 empty-env guard). The P1 stories together form a viable MVP — the command can be shipped with just P1 + the safety story.
- Each FR maps to at least one acceptance scenario or edge case. The traceability is implicit but consistent.
- The Out of Scope section is explicit and exhaustive: it lists the six items that were deliberately deferred and explains why each is a follow-up.

### Assumptions Made

1. The user is allowed to use the existing per-env passphrase env-var naming convention (`ENVY_PASSPHRASE_<ENV>`) for the current passphrase and adds a `_NEW` suffix for the new passphrase. This is consistent with the rest of envy's headless patterns.
2. The existing `confirm_key_rotation` interactive prompt in `envy encrypt` is kept (not removed) for backward compatibility.
3. The vault is the source of truth for secret values; the artifact is rebuilt from the vault and re-sealed with the new passphrase. This is consistent with `envy encrypt`'s existing behaviour.
4. The "forward-only" semantics are documented as the intended behaviour and as the reason this is not a revocation tool. The spec does not promise to add revocation in a future iteration.

### Items NOT Requiring Follow-up

No clarifications were needed; no items are flagged as incomplete. The spec is ready for the planning phase.
