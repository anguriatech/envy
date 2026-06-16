# Feature Specification: Strict `envy encrypt` (No Silent Key Rotation)

**Feature Branch**: `013-encrypt-strict`
**Created**: 2026-06-10
**Status**: Draft
**Input**: User description: "Make `envy encrypt` STRICT so it can no longer silently rotate the passphrase of an existing envelope. Instead, the user's intent is unambiguous: `envy encrypt` always seals the vault contents with a passphrase that EITHER matches the existing envelope (update) OR is the first time an envelope is created (new). If the passphrase does not match the existing envelope, `envy encrypt` MUST fail with a clear error and a hint pointing to `envy rotate`. This closes the "silent key rotation" gap left after spec 012 introduced `envy rotate` as the dedicated, safe path for key rotation."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - First-time seal of a new envelope (Priority: P1)

A developer (Ana) is starting a new project. She runs `envy init`, sets a couple of secrets with `envy set`, and then runs `envy encrypt -e production`. There is no envelope in `envy.enc` yet for `production`, so the CLI prompts her for a new passphrase with double-entry confirmation (the same UX as today). She enters and confirms the new passphrase, and the envelope is created and written atomically to `envy.enc`. She commits and pushes. A teammate pulls, runs `envy decrypt` with the same passphrase, and gets the secrets.

**Why this priority**: This is the existing happy path for `envy encrypt` and must continue to work. The whole point of this spec is to preserve the success cases while removing the silent-rotation case.

**Independent Test**: Can be tested end-to-end on a fresh temporary project: init → set → encrypt (interactive or headless via `ENVY_PASSPHRASE_PRODUCTION=...`). The envelope is created; `envy decrypt` with the same passphrase imports the secrets; the SHA-256 of `envy.enc` is non-empty and parseable.

**Acceptance Scenarios**:

1. **Given** a fresh project with secrets in the local vault for `production` and no envelope in `envy.enc` yet, **When** the user runs `envy encrypt -e production` interactively and enters a new passphrase (with confirmation), **Then** the envelope is created and the CLI prints a success message.
2. **Given** a fresh project with secrets in the local vault for `production` and no envelope in `envy.enc` yet, **When** the user runs `envy encrypt -e production` headlessly with `ENVY_PASSPHRASE_PRODUCTION` set, **Then** the envelope is created and the CLI prints a success message.

---

### User Story 2 - Update seal with a matching passphrase (Priority: P1)

A developer (Bea) is on a project where `production` was already sealed with passphrase `A`. She has made changes to the local vault (added a new secret) and runs `envy encrypt -e production`. She is prompted for the CURRENT passphrase (one prompt, no double entry — the user is verifying, not creating). She enters `A`; the CLI verifies it matches the existing envelope, then re-seals the envelope with the same passphrase `A` (a fresh salt and nonce are generated, but the passphrase is unchanged). She commits and pushes. A teammate who already has `A` decrypts normally.

**Why this priority**: This is the second existing happy path. The matching-passphrase update case must continue to work seamlessly. The user expects "I re-ran encrypt and it worked" to keep working.

**Independent Test**: Seal an envelope with passphrase `A`, change a local secret, re-seal with `A` (interactive prompt or `ENVY_PASSPHRASE_PRODUCTION=A`). The envelope is re-created with a fresh salt + nonce but the same passphrase; `envy decrypt` with `A` works; `envy decrypt` with any other passphrase fails (progressive disclosure).

**Acceptance Scenarios**:

1. **Given** an envelope exists in `envy.enc` sealed with passphrase `A`, **When** the user runs `envy encrypt -e production` interactively and enters `A`, **Then** the CLI verifies, re-seals, and prints a success message.
2. **Given** an envelope exists in `envy.enc` sealed with passphrase `A`, **When** the user runs `envy encrypt -e production` headlessly with `ENVY_PASSPHRASE_PRODUCTION=A`, **Then** the CLI verifies, re-seals, and prints a success message. The fresh envelope is byte-different from the old one (new salt, new nonce) but decrypts with the same passphrase.
3. **Given** an envelope exists in `envy.enc` sealed with passphrase `A`, **When** the user has made NO local changes but still runs `envy encrypt -e production` with `A`, **Then** the envelope is re-sealed (this is "I re-ran encrypt to bump the marker / refresh the artifact" — a legitimate use case that must continue to work).

---

### User Story 3 - Mismatch with existing envelope fails clearly (Priority: P1)

A developer (Carles) is on a project where `production` was already sealed with passphrase `A`, but he has only passphrase `B` (perhaps a teammate shared the wrong one, or he typoed a CI var). He runs `envy encrypt -e production`. The CLI MUST NOT silently re-seal the envelope with `B` (which would lock the rest of the team out of the artifact). Instead, the CLI MUST fail with exit code 2 and a clear error message that points the user to `envy rotate`.

**Why this priority**: This is the safety invariant that the spec exists to enforce. If the mismatch case does not fail clearly, the spec is not delivered.

**Independent Test**: Seal an envelope with passphrase `A`, then run `envy encrypt -e production` with passphrase `B` (both interactive and headless). The CLI exits 2; the error message includes the hint `use envy rotate -e ENV to change the envelope's passphrase`; the SHA-256 of `envy.enc` is byte-identical before and after the attempt; the local vault is unchanged.

**Acceptance Scenarios**:

1. **Given** an envelope exists in `envy.enc` sealed with passphrase `A`, **When** the user runs `envy encrypt -e production` interactively and enters a wrong passphrase `B`, **Then** the CLI exits 2 with the exact error message `error: passphrase does not match the existing envelope.\nhint: use envy rotate -e ENV to change the envelope's passphrase.`
2. **Given** an envelope exists in `envy.enc` sealed with passphrase `A`, **When** the user runs `envy encrypt -e production` headlessly with `ENVY_PASSPHRASE_PRODUCTION=B` (wrong), **Then** the CLI exits 2 with the same error message and the artifact is byte-identical to its pre-attempt state.
3. **Given** the CI script sets `ENVY_PASSPHRASE=WRONG` (global, no per-env suffix) and runs `envy encrypt -e production`, **When** the existing envelope was sealed with a different passphrase, **Then** the CLI exits 2 with the same error message. This is the breaking change vs v0.3.0 (was silent rotation).
4. **Given** the mismatch case ran, **When** the user inspects `envy.enc` afterwards, **Then** the file is byte-identical to the pre-attempt state (verified by SHA-256 before and after the attempt).

---

### User Story 4 - Empty-vault guard (unchanged from today) (Priority: P2)

A developer runs `envy encrypt -e empty-env` when the local vault has zero secrets for that environment. The CLI skips the seal with a warning and exits 0. The artifact is not modified. This is the existing behaviour and must continue to work.

**Why this priority**: This is a guard rail that already exists; the spec must not break it. The "create" path (Case 1) and the "update" path (Case 2) both apply this guard before the verify step.

**Independent Test**: Create an env row in the vault with zero secrets, then run `envy encrypt -e empty-env` (both interactive and headless). The CLI prints a yellow warning, exits 0, and `envy.enc` is unchanged.

**Acceptance Scenarios**:

1. **Given** an env row exists in the vault but has zero secrets, **When** the user runs `envy encrypt -e empty-env` interactively, **Then** the CLI prints a warning and exits 0 without modifying the artifact.
2. **Given** an env row exists in the vault but has zero secrets, **When** the user runs `envy encrypt -e empty-env` headlessly, **Then** the CLI prints a warning and exits 0 without modifying the artifact.

---

### User Story 5 - First-time seal with empty vault (NEW consistency rule) (Priority: P2)

In the v0.3.0 behaviour, a "first-time seal with an empty vault" was rejected by the empty-vault guard for the new-env case. In the v0.3.0 behaviour, an "update seal with an empty vault" (the user has 0 secrets locally and runs encrypt against an existing envelope) would still seal an empty envelope. The v0.3.1 spec MUST apply the empty-vault guard in BOTH cases for consistency: an update seal with an empty local vault MUST skip with a warning rather than sealing an empty envelope over the existing one.

**Why this priority**: This is a small consistency tightening that prevents the user from accidentally wiping the contents of an envelope by running `envy encrypt` in the wrong directory or after deleting all local secrets. The risk is low (the user still has the original `envy.enc` in git), but the consistency is worth enforcing.

**Independent Test**: Seal an envelope with passphrase `A` and one secret. Locally, delete that secret (so the vault has 0 secrets for the env). Run `envy encrypt -e ENV` with passphrase `A`. The CLI prints the empty-vault warning and exits 0; the artifact is unchanged; `envy decrypt` with `A` still works.

**Acceptance Scenarios**:

1. **Given** an envelope exists in `envy.enc` sealed with passphrase `A` and one secret, **When** the user locally deletes the only secret and runs `envy encrypt -e ENV` with `A`, **Then** the CLI prints the empty-vault warning and exits 0; the artifact is unchanged; `envy decrypt` with `A` still returns the original secret.

---

### Edge Cases

- **What happens when the user provides the new passphrase equal to the existing passphrase (no real change) in the new-envelope case?**
  The CLI proceeds with the seal. There is no constraint that the new passphrase must differ from anything; this is a first-time seal, so any non-empty passphrase is acceptable.

- **What happens when the user provides the current passphrase equal to the new passphrase in the update case?**
  The CLI proceeds. Re-sealing with the same passphrase is the legitimate "I re-ran encrypt to refresh the artifact" use case.

- **What happens when `envy.enc` does not exist at all?**
  The CLI proceeds with the first-time seal (treats every requested env as a new envelope).

- **What happens when the env in `-e` is not in the vault at all (no env row)?**
  This is the same as the empty-vault case: the CLI skips with a warning and exits 0. The user must run `envy set` first to create the env row and the secrets.

- **What happens when the user is in interactive mode but Ctrl-C's during the passphrase prompt?**
  The CLI exits 2 with no change to `envy.enc`. This is the existing behaviour of `resolve_passphrase_for_env` and is unchanged.

- **What happens when the passphrase env var is set to whitespace only?**
  The CLI exits 2 with a clear error (whitespace-only passphrases are not accepted anywhere in envy). This is the existing behaviour and is unchanged.

- **What happens when the global `ENVY_PASSPHRASE` env var (no per-env suffix) is set in interactive mode?**
  In interactive mode, the prompt takes precedence. The global env var is only used in headless mode. This is the existing behaviour and is unchanged.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `envy encrypt` subcommand MUST NOT silently re-seal an existing envelope with a different passphrase.
- **FR-002**: If the target envelope does not exist in `envy.enc` (first-time seal), the CLI MUST accept the user-supplied passphrase and create the envelope.
- **FR-003**: If the target envelope exists in `envy.enc` and the user-supplied passphrase matches the existing envelope, the CLI MUST re-seal the envelope with the same passphrase.
- **FR-004**: If the target envelope exists in `envy.enc` and the user-supplied passphrase does NOT match the existing envelope, the CLI MUST exit 2 with the exact error message `error: passphrase does not match the existing envelope.\nhint: use envy rotate -e ENV to change the envelope's passphrase.`
- **FR-005**: When the mismatch case is triggered, the CLI MUST NOT modify `envy.enc` (the atomic write helper is not even called) and MUST NOT modify the local vault.
- **FR-006**: The empty-vault guard MUST apply to BOTH the new-envelope case and the update-envelope case. If the local vault has zero secrets for the target env, the CLI MUST skip the seal with a warning and exit 0, regardless of whether the envelope exists in `envy.enc`.
- **FR-007**: The Diceware passphrase suggestion for new envelopes (the `suggest_passphrase(4)` flow) MUST be preserved. When a new envelope is being created interactively, the existing Diceware suggestion continues to work.
- **FR-008**: The `sync_markers` table MUST be updated on every successful seal (unchanged behaviour; the existing `seal_env` helper in `core::sync.rs` already writes the marker).
- **FR-009**: The atomic write of `envy.enc` (the existing `write_artifact_atomic` helper) MUST be used for every successful seal.
- **FR-010**: The global `ENVY_PASSPHRASE` env var (no per-env suffix) MUST continue to be honoured as the passphrase in headless mode, but with the same strict verify-or-fail behaviour. This preserves backward compatibility with existing CI scripts.
- **FR-011**: The `confirm_key_rotation` interactive prompt MUST be removed from the codebase. The `envy encrypt` command MUST NOT prompt the user to "rotate the key" when the passphrase does not match.
- **FR-012**: The CLI MUST exit 2 for the empty-passphrase, confirmation-mismatch, and wrong-passphrase cases (all passphrase-input failures map to the existing `PassphraseInput` error variant, which already maps to exit 2).

### Key Entities *(include if feature involves data)*

- **Sealed envelope** (existing, behaviour-tightened): a single environment's encrypted blob in `envy.enc`. The `envy encrypt` command's relationship to an existing envelope is now restricted to two cases: (a) the envelope does not exist → create; (b) the envelope exists and the passphrase matches → re-seal. The third case (envelope exists, passphrase does not match) is no longer reachable in `envy encrypt`; it is the responsibility of `envy rotate`.
- **Passphrase env vars** (existing, behaviour-preserved): `ENVY_PASSPHRASE` (global) and `ENVY_PASSPHRASE_<ENV>` (per-env) continue to work as before, but now trigger the strict verify-or-fail behaviour.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user running `envy encrypt -e production` in a fresh project is prompted for a new passphrase (with confirmation) and the envelope is created. The artifact is written and `envy decrypt` with the same passphrase returns the secrets.
- **SC-002**: A user running `envy encrypt -e production` in a project with an existing envelope and the correct passphrase sees the envelope re-sealed. The fresh envelope is byte-different from the old one (new salt, new nonce) but decrypts with the same passphrase.
- **SC-003**: A user running `envy encrypt -e production` in a project with an existing envelope and an incorrect passphrase (interactive OR headless) sees the exact error message `passphrase does not match the existing envelope. use envy rotate -e ENV to change the envelope's passphrase.` and the CLI exits 2. The SHA-256 of `envy.enc` before the attempt is identical to the SHA-256 after the attempt.
- **SC-004**: A CI script that sets `ENVY_PASSPHRASE=correct-pass` and runs `envy encrypt -e production` continues to work (no behaviour change for the matching-passphrase case).
- **SC-005**: A CI script that sets `ENVY_PASSPHRASE=WRONG-pass` and runs `envy encrypt -e production` now FAILS with exit 2 (was silently rotating in v0.3.0). This is the breaking change that the spec exists to enforce.
- **SC-006**: The `confirm_key_rotation` function is removed from the codebase. A grep for the function name returns zero matches in the source code.
- **SC-007**: The pre-existing tests for `envy rotate` (spec 012) continue to pass without modification, demonstrating that the new strict-encrypt behaviour does not affect the dedicated rotation path.
- **SC-008**: The full E2E suite (10+ scenarios) passes. Any scenario that previously relied on the "wrong passphrase = silent rotation" behaviour is updated to expect exit 2 + the new error message.
- **SC-009**: The full unit test suite passes with the test changes described in the acceptance criteria. Any test that asserted "silent rotation in headless mode" is updated to assert "rejected with `PassphraseInput` error in headless mode".

## Out of Scope *(deferred — documented as follow-ups)*

The following items are explicitly out of scope for this spec and are documented here so that future specs can pick them up. They are not part of the acceptance criteria.

- **Removing `envy rotate`'s interactive prompt**: the prompt is appropriate because `envy rotate` is the dedicated, safe path for key rotation. The user is explicitly choosing to rotate, and the prompt provides a clear "old → new → confirm" flow.
- **Adding a `--force-rotate` flag to `envy encrypt`**: not needed. Users who want to rotate have `envy rotate`.
- **Changing `envy decrypt` behaviour**: the decrypt command already handles mismatch gracefully (Progressive Disclosure — wrong passphrase = skip the envelope). No change needed.
- **Changes to the VS Code extension**: the extension is a follow-up spec. After this CLI change ships, the extension's first marketplace release is the next milestone.
- **Removing the key-rotation warning prompt in `envy encrypt`**: this prompt is being removed by this spec (it is the only call site of `confirm_key_rotation`). Documented as a deliberate behaviour tightening, not as a deferred follow-up.
- **Adding a `--force` flag to `envy encrypt` to override the strict check**: explicitly out of scope. The whole point of this spec is that there is no override; users have `envy rotate` for the rotation case.

## Behavioural Change Table *(informational, not acceptance criteria)*

| Scenario | Old (v0.3.0) | New (v0.3.1) |
|----------|---------------|----------------|
| First-time seal, interactive | prompt new pp + double entry | unchanged |
| First-time seal, headless | use env var | unchanged |
| First-time seal, vault empty | skip with warning | unchanged |
| Update seal, passphrase matches | seal | unchanged |
| Update seal, mismatch, interactive | warn + ask confirmation + rotate | **fail with exit 2, hint** |
| Update seal, mismatch, headless | silent rotation | **fail with exit 2, hint** |
| Update seal, vault empty | seal empty envelope | **skip with warning** (consistent with new-env case) |
| `ENVY_PASSPHRASE` global env var, headless | silently rotate on mismatch | **fail with exit 2, hint** |
| `ENVY_PASSPHRASE` global env var, interactive | prompt | unchanged (interactive uses the prompt) |

## Versioning

- Bump `Cargo.toml` from `0.3.0` to `0.3.1` (patch bump on the minor). Rationale: this spec tightens the contract of an existing command but does not introduce new public API. Per the project's pre-1.0 versioning convention, behaviour tightenings on existing commands are patch bumps.
- Document the behaviour change in the commit message: `envy encrypt: passphrase mismatch with existing envelope now fails with exit 2 (was: silent rotation in headless mode). Use envy rotate to change the envelope's passphrase.`
- The project's first non-prerelease version is deferred until the VS Code extension is published and the contract is stable across one full release cycle. That release can be tagged as `0.4.0` or `1.0.0-rc.1`.

## Notes for Future Specs

- The spec 006 acceptance scenarios about "key rotation warning" in `envy encrypt` are now obsolete. The spec 006 design assumptions are still valid (e.g., "the CLI should warn before doing something destructive"); only the implementation in `envy encrypt` has changed. Spec 006 itself does not need to be re-written; the spec is the *user-visible behaviour*, and the user-visible behaviour is now correctly documented in spec 013.
- The pre-1.0 status of the project is a deliberate choice that makes this breaking change acceptable. Once a non-prerelease version is published, future breaking changes will require a deprecation cycle.
