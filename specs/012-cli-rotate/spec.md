# Feature Specification: Envelope Passphrase Rotation

**Feature Branch**: `012-cli-rotate`
**Created**: 2026-06-10
**Status**: Draft
**Input**: User description: "Add a dedicated `envy rotate` command to the CLI (specs/012-cli-rotate) that lets the user explicitly rotate the passphrase of a sealed envelope. The rotation MUST verify the OLD passphrase before accepting the new one so typos cannot silently produce a key rotation."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Single-environment passphrase rotation (Priority: P1)

A tech lead (Ana) needs to rotate the passphrase of the `production` envelope in `envy.enc` because the existing passphrase was shared over a channel that is no longer trusted. She runs `envy rotate -e production`, enters the current passphrase, types a new passphrase, confirms it, and the envelope is re-sealed with the new passphrase. She then commits the updated `envy.enc` to the repository and shares the new passphrase through the team's password manager. The git diff of `envy.enc` shows the rotation took effect; artifacts sealed with the old passphrase can no longer be decrypted.

**Why this priority**: This is the core use case. Without this flow, the team has no safe way to rotate a passphrase. All other stories (multi-env, headless) build on this.

**Independent Test**: Can be tested end-to-end on a temporary project: seal an envelope with passphrase A, run `envy rotate -e ENV` with passphrase A and a new passphrase B, then attempt to decrypt with A (must fail) and with B (must succeed). The envelope's contents and the vault are unchanged across the rotation; only the sealing passphrase changes.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` containing a sealed `production` envelope, **When** the user runs `envy rotate -e production` and enters the correct current passphrase and a new passphrase (twice), **Then** the envelope is re-sealed with the new passphrase and the CLI prints a clear success message that includes a note that the rotation is forward-only.
2. **Given** the user has just rotated `production`, **When** they run `envy decrypt` with the OLD passphrase, **Then** the `production` envelope is skipped (consistent with existing progressive-disclosure behavior) and the other envelopes are still imported normally.
3. **Given** the user has just rotated `production`, **When** they run `envy decrypt` with the NEW passphrase, **Then** the `production` envelope is imported successfully.

---

### User Story 2 - Rotation fails safely on a wrong current passphrase (Priority: P1)

A user (Bea) attempts to rotate the `production` envelope but mistypes the current passphrase. The CLI must NOT silently re-seal the envelope with a different passphrase. Instead, the CLI must reject the attempt, leave both `envy.enc` and the vault untouched, and report a clear error explaining what went wrong.

**Why this priority**: This is the primary safety guarantee of the new command. The whole reason the command exists is to prevent silent rotation from a typo. If this scenario is not handled correctly, the command is worse than the status quo.

**Independent Test**: Can be tested by attempting to rotate with a wrong current passphrase and verifying that the artifact file (and its contents) are byte-identical before and after the failed rotation. The vault is also unchanged.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` with a sealed envelope, **When** the user runs `envy rotate -e ENV` and enters a wrong current passphrase followed by a new passphrase (twice), **Then** the CLI exits with a clear error (no envelope is re-sealed, the artifact and vault are unchanged, the exit code is 2).
2. **Given** the user has been prompted for the current passphrase, **When** they realise they do not know the correct current passphrase and abort (e.g. Ctrl-C), **Then** the rotation is cancelled and the artifact and vault are unchanged.
3. **Given** a wrong-current-passphrase attempt, **When** the user inspects the artifact afterwards, **Then** the file contents are byte-identical to the pre-attempt state.

---

### User Story 3 - Headless rotation in CI / scripted workflows (Priority: P2)

A CI pipeline must rotate the passphrase of a sealed envelope as part of an automated rotation job. There is no human in front of a terminal, so the user provides both the current and the new passphrase via environment variables. The CLI must run without prompting, perform the rotation atomically, and exit with a clear success or failure code.

**Why this priority**: This is the primary alternative path to interactive rotation and is essential for teams that automate their secret lifecycle. It is not P1 because the command must be usable in P1 without CI integration.

**Independent Test**: Can be tested by running the command with both `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` set, with no TTY attached. The rotation must complete without any prompt being displayed and the artifact must be updated.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` with a sealed envelope, **When** the user runs `envy rotate -e ENV` with both `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` set in the environment and no TTY, **Then** the rotation completes successfully, the CLI prints a success line, and the artifact is updated.
2. **Given** a headless rotation is requested, **When** the wrong current passphrase is supplied via the env var, **Then** the CLI exits with a clear error and the artifact is unchanged.
3. **Given** a headless rotation is requested, **When** the new passphrase equals the current passphrase (both supplied via env vars), **Then** the CLI exits with a clear error and the artifact is unchanged.
4. **Given** neither a TTY nor the required env vars is available, **When** the user runs `envy rotate -e ENV`, **Then** the CLI exits with a clear error message stating that `envy rotate` requires either a TTY or the appropriate env vars.
5. **Given** the global `ENVY_PASSPHRASE` env var (without a per-env suffix) is set, **When** the user runs `envy rotate -e ENV`, **Then** the CLI does NOT silently use it as the current passphrase; rotation requires either a TTY or the explicit per-env env vars.

---

### User Story 4 - Multi-environment rotation (Priority: P2)

A user wants to rotate several envelopes in one operation (e.g. rotate `development` and `staging` together, leaving `production` untouched). They run `envy rotate` without `-e` and are presented with a multi-select prompt listing all envelopes that exist in `envy.enc`. After selection, the CLI prompts for the current and new passphrase for each selected environment in turn.

**Why this priority**: Multi-env support is a quality-of-life feature that mirrors the existing `envy encrypt` behaviour. It is not P1 because users can rotate one environment at a time with P1.

**Independent Test**: Can be tested by selecting multiple environments in the multi-select prompt, entering the passphrases for each, and verifying that all selected envelopes are re-sealed and the unselected ones are unchanged.

**Acceptance Scenarios**:

1. **Given** an `envy.enc` with multiple sealed envelopes, **When** the user runs `envy rotate` and selects more than one environment, **Then** the CLI prompts for the current and new passphrase for each selected environment in turn, and re-seals each.
2. **Given** a multi-environment rotation is in progress, **When** one of the selected environments has a wrong current passphrase, **Then** that environment is skipped with a warning and the others continue to be rotated.
3. **Given** a multi-environment headless rotation is requested (env vars for several envs are set), **When** only some of those envs have both `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` set, **Then** the envs that have both vars are rotated and the others are skipped with a warning.

---

### User Story 5 - Empty-envelope guard (Priority: P3)

A user attempts to rotate the passphrase of an environment whose local vault contains zero secrets. The CLI must skip the rotation with a warning, mirroring the existing behaviour of `envy encrypt` (which also skips empty environments).

**Why this priority**: This is a guard rail that prevents the user from creating a meaningless `envy.enc` change. It is low priority because it does not block the primary use cases.

**Independent Test**: Can be tested by running the rotation against an environment with no secrets in the local vault and verifying that the envelope in `envy.enc` is unchanged and a warning is printed.

**Acceptance Scenarios**:

1. **Given** an environment exists in `envy.enc` but has zero secrets in the local vault, **When** the user runs `envy rotate -e ENV`, **Then** the CLI prints a warning that the environment is empty and skips the rotation; the artifact is unchanged.

---

### Edge Cases

- **What happens when the user enters the new passphrase equal to the current passphrase?**
  The CLI exits with a clear error and leaves the artifact unchanged. The error message must explain why the new passphrase must differ.

- **What happens when the user enters two different passphrases at the confirmation step?**
  The CLI exits with a clear "passphrases do not match" error and leaves the artifact unchanged.

- **What happens when the user enters a whitespace-only new passphrase?**
  The CLI exits with a clear error (whitespace-only passphrases are not accepted anywhere in envy) and leaves the artifact unchanged.

- **What happens when `envy.enc` does not exist?**
  The CLI exits with a clear error explaining that there is no envelope to rotate and suggesting `envy encrypt -e ENV` as the way to create one. Exit code 1.

- **What happens when `envy.enc` exists but does not contain the requested environment?**
  The CLI exits with a clear error explaining that the environment is not present in the artifact. Exit code 1.

- **What happens when the user requests rotation but the local vault contains a different number of secrets for that environment than the artifact?**
  The rotation re-seals whatever the local vault currently holds. The behavior is consistent with `envy encrypt` — the artifact is rebuilt from the local vault.

- **What happens if the rotation succeeds but the artifact write fails partway through?**
  The artifact file is either the pre-rotation version or the post-rotation version, never a partial one. A temporary file may be created during the operation but is cleaned up on success and does not replace the original on failure.

- **What happens if the rotation succeeds but the user interrupts the process (Ctrl-C) after the artifact has been written?**
  The rotation is complete; the artifact is in its post-rotation state. There is no "transaction" with the vault, only with the artifact file.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CLI MUST provide a `rotate` subcommand.
- **FR-002**: The `rotate` subcommand MUST accept an optional `-e` / `--env` flag with a single environment name. When the flag is absent, the CLI MUST prompt the user to select one or more environments from the envelopes that exist in `envy.enc` (mirroring the multi-select behaviour of `envy encrypt`).
- **FR-003**: When the user provides a single environment via `-e`, the CLI MUST prompt for the current passphrase, the new passphrase, and a confirmation of the new passphrase (in that order) before performing any change.
- **FR-004**: When the user provides multiple environments (via the multi-select prompt), the CLI MUST prompt for the current, new, and confirmation passphrases for each selected environment in turn, in a stable order.
- **FR-005**: The CLI MUST verify the current passphrase by attempting to unseal the existing envelope before accepting the new passphrase. If unsealing fails, the CLI MUST abort the rotation for that environment with a clear error and leave the artifact byte-identical to its prior state.
- **FR-006**: The CLI MUST reject a new passphrase that equals the current passphrase with a clear error and must leave the artifact unchanged.
- **FR-007**: The CLI MUST reject a confirmation that does not match the new passphrase with a clear error and must leave the artifact unchanged.
- **FR-008**: The CLI MUST reject whitespace-only new passphrases with a clear error and must leave the artifact unchanged.
- **FR-009**: If the environment does not exist in `envy.enc`, the CLI MUST abort with a clear error explaining that the user must run `envy encrypt -e ENV` first to create the envelope. Exit code 1.
- **FR-010**: If the local vault contains zero secrets for the target environment, the CLI MUST skip the rotation for that environment with a warning and continue with the next (or, if it is the only one, exit successfully with a warning).
- **FR-011**: The CLI MUST support a headless mode triggered by the simultaneous presence of `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` in the environment. In headless mode, the CLI MUST NOT display any prompt and MUST use the env-var values as the current and new passphrases respectively.
- **FR-012**: The CLI MUST NOT honour the global `ENVY_PASSPHRASE` env var (without a per-env suffix) for the rotation command. Rotation must be explicit, either via a TTY prompt or via the per-env pair of env vars.
- **FR-013**: If neither a TTY nor the required env vars are available, the CLI MUST abort with a clear error stating that `envy rotate` requires either a TTY or `ENVY_PASSPHRASE_<ENV>` + `ENVY_PASSPHRASE_<ENV>_NEW`.
- **FR-014**: In multi-environment headless mode, the CLI MUST process each environment that has both env vars set and MUST skip the others with a warning.
- **FR-015**: On successful rotation, the CLI MUST print a clear success message that includes an explicit note that the rotation is forward-only: the old passphrase can no longer decrypt the artifact, and any artifacts sealed with the old passphrase are permanently invalid.
- **FR-016**: On successful rotation, the CLI MUST write the updated `envy.enc` atomically (a temporary file is created and then atomically replaces the original; on failure, the original is preserved and the temporary file is cleaned up).
- **FR-017**: On successful rotation, the CLI MUST update the artifact's per-envelope metadata in a way that downstream tools (e.g. `envy status`) can use to determine that the envelope has been re-sealed.
- **FR-018**: The CLI MUST preserve all existing envelopes in `envy.enc` that are not part of the rotation. Only the targeted envelopes are re-sealed.
- **FR-019**: The CLI MUST use exit code 0 on success, 1 on "not found" (missing artifact or missing envelope in artifact), 2 on invalid input (wrong current passphrase, confirmation mismatch, new = current, no TTY and no env vars).
- **FR-020**: The `rotate` subcommand MUST NOT modify the local vault. It only re-seals the envelope in `envy.enc`. The vault's state is the source of truth for the secrets; the artifact is rebuilt from the vault and re-sealed with the new passphrase.

### Key Entities *(include if feature involves data)*

- **Sealed envelope**: A single environment's encrypted blob in `envy.enc`. It contains the ciphertext, a nonce, the per-envelope sealing passphrase material, and a timestamp indicating when the envelope was last re-sealed. The envelope's passphrase is what `envy rotate` re-derives. The envelope's plaintext (the actual key-value pairs) is sourced from the local vault and is not affected by the rotation.

- **Artifact (`envy.enc`)**: A repository-committed file that bundles one envelope per environment. The rotation re-seals one or more envelopes inside the artifact and writes the artifact back atomically. Envelopes that are not part of the rotation are preserved unchanged.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can rotate the passphrase of a single environment end-to-end (typing the current and new passphrases through interactive prompts) in under 30 seconds.
- **SC-002**: An attempt to rotate with a wrong current passphrase leaves the artifact byte-identical to its pre-attempt state (verified by a SHA-256 of the file before and after the attempt).
- **SC-003**: After a successful rotation, attempting to decrypt the same environment with the OLD passphrase fails with a clear error, and attempting to decrypt with the NEW passphrase succeeds and yields the exact same set of secrets as before the rotation (verified by comparing the count and the values).
- **SC-004**: The pre-existing tests for `envy encrypt` and `envy decrypt` continue to pass without modification, demonstrating that rotation is an additive feature.
- **SC-005**: A headless rotation (CI scenario) completes in a single invocation without prompting, given both `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` are set, and the exit code is 0 on success / 2 on any input-related failure.
- **SC-006**: A multi-environment rotation (interactive, selecting two or more environments) successfully re-seals only the selected environments and leaves the others byte-identical, verified by computing a SHA-256 of each envelope's contents before and after.
- **SC-007**: An attempt to rotate an environment that has no secrets in the local vault exits with a warning and does NOT modify the artifact (verified by SHA-256 of the file before and after the attempt).

## Out of Scope *(deferred — documented as follow-ups)*

The following items are explicitly out of scope for this spec and are documented here so that future specs can pick them up. They are not part of the acceptance criteria.

- **Rotation audit log**: there is no historical record inside `envy.enc` of who rotated what and when. The git commit timestamp and author on `envy.enc` are the only audit trail. A follow-up spec could add a separate audit envelope or an external rotation log.
- **Auto-rotation on a schedule**: there is no mechanism to schedule rotations or to remind users that a passphrase is "old". This is a product-level concern, not a CLI concern.
- **Revocation**: `envy rotate` is forward-only. It does not support the more advanced "this passphrase has been compromised, reject it" semantic. The intended use of rotation is to migrate the team's knowledge of the passphrase, not to invalidate an exposed one. The spec explicitly states that `envy rotate` is NOT a revocation tool.
- **Changes to `envy encrypt`'s silent rotation behaviour**: the existing behaviour in headless mode (where `envy encrypt` silently rotates when the passphrase does not match the existing envelope) is preserved for backward compatibility. The new `envy rotate` command is the safe path; the old behaviour is not modified by this spec.
- **Centralised distribution of the new passphrase**: the CLI does not solve how the team shares the new passphrase. This is the team's responsibility (password manager, secure channel, etc.). The spec notes this as a required team process but does not implement it.
- **Removal of the existing key-rotation warning prompt in `envy encrypt`**: that prompt becomes redundant once `envy rotate` exists, but it is preserved for backward compatibility in this spec. A follow-up could remove it once `envy rotate` is widely adopted.
