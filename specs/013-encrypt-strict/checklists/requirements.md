# Specification Quality Checklist: Strict `envy encrypt` (No Silent Key Rotation)

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
- The spec describes the `envy encrypt` change in terms of user-visible behaviour: success cases, failure cases, exit codes, error messages. It does not mention Rust, crates, modules, AES-GCM, SQLCipher, dialoguer, `core::check_envelope_passphrase`, or any other implementation detail. (The Key Entities section mentions "existing helpers" by name as a hint to the planner, but the spec itself does not depend on those names being accurate — the *behaviour* the helpers provide is what matters.)
- Acceptance scenarios are written in business language (e.g. "A developer runs `envy encrypt -e production` and enters a wrong passphrase — the CLI exits 2 with a clear error message"), not in technical terms.
- All mandatory sections (User Scenarios & Testing, Requirements, Success Criteria) are present and non-empty.

### Requirement Completeness
- No `[NEEDS CLARIFICATION]` markers were used. The user description was extremely detailed and left no critical ambiguities. Three "informed-guess" decisions were made and documented in the spec's Notes section:
  1. The exact error message format (`error: passphrase does not match the existing envelope.\nhint: use envy rotate -e ENV to change the envelope's passphrase.`) — derived directly from the user's prose.
  2. The consistency rule that the empty-vault guard now applies in BOTH the new-env and update-env cases (User Story 5) — derived from the "consistent with new-envelope case" hint in the behavioural-change table.
  3. The version bump from 0.3.0 to 0.3.1 (patch) — derived from the user's explicit "Versioning" section.
- Every FR is testable. For example, FR-004 (mismatch case error message) is testable by attempting a wrong passphrase and asserting the exact message text + exit code 2. This is the SC-003 success criterion.
- Success criteria are measurable: SHA-256 before/after (SC-003, SC-005), exact error message text (SC-003), exit code (SC-001 through SC-005), `grep` for `confirm_key_rotation` returning zero matches (SC-006), test suite pass count (SC-008, SC-009).
- Edge cases include: new passphrase = current passphrase (no-op re-seal), Ctrl-C during prompt, whitespace-only env var, env not in vault, global `ENVY_PASSPHRASE` in interactive mode.

### Feature Readiness
- The five user stories cover the primary flows:
  - US1 (P1): first-time seal, interactive + headless
  - US2 (P1): update seal, matching passphrase, interactive + headless
  - US3 (P1): mismatch case (the breaking change)
  - US4 (P2): empty-vault guard, unchanged
  - US5 (P2): empty-vault guard applied to update case (new consistency rule)
- The P1 stories together form the viable MVP — the spec is shippable with just US1, US2, and US3. US4 and US5 are consistency tightenings that prevent user errors.
- Each FR maps to at least one acceptance scenario or edge case. Traceability is implicit but consistent.
- The Out of Scope section is explicit and exhaustive: it lists the six items that were deliberately deferred and explains why each is a follow-up or a deliberate non-goal.

### Assumptions Made

1. The exact error message text is fixed in the spec (`error: passphrase does not match the existing envelope.\nhint: use envy rotate -e ENV to change the envelope's passphrase.`) — the user provided this exact format in the proposal.
2. The empty-vault guard is applied in BOTH cases (new-envelope and update-envelope) — this is a small consistency tightening that was inferred from the user's "consistent with new-envelope case" note in the behavioural-change table. The user did not explicitly call this out, but the consistency rule is the natural reading of the change.
3. The version bump is 0.3.0 → 0.3.1 (patch) — the user provided this in the "Versioning" section.
4. The `confirm_key_rotation` function and its call site are the ONLY changes required to `cmd_encrypt` — the user did an explicit `grep`-style audit (in the proposal) and listed the specific code locations to modify.
5. The Diceware suggestion flow for new envelopes is preserved unchanged — the user explicitly listed this in "What MUST stay".
6. The `envy rotate` command (spec 012) is unchanged — the user explicitly listed this in "What MUST stay".
7. The pre-existing `resolve_passphrase_for_env` helper is reused for the new-envelope case (with `confirm=true` for new envelopes, `confirm=false` for update envelopes) — this is the natural factoring given the existing helper's API.

### Items NOT Requiring Follow-up

No clarifications were needed; no items are flagged as incomplete. The spec is ready for the planning phase.
