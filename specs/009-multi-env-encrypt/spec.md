# Feature Specification: Multi-Environment Encryption and Smart Merging

**Feature Branch**: `009-multi-env-encrypt`
**Created**: 2026-03-25
**Status**: Draft
**Input**: Issue #2 — overhaul `envy encrypt` to support multi-environment selection, smart merging, key-rotation protection, and headless CI mode.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — CI/CD Headless Encryption (Priority: P1)

A CI pipeline encrypts one or more environments using environment variables, with zero interaction. The operator sets `ENVY_PASSPHRASE_PRODUCTION=<secret>` (or `ENVY_PASSPHRASE` as a fallback) in the pipeline secrets, then runs `envy encrypt`. No prompts appear; the artifact is sealed and written atomically.

**Why this priority**: Unblocks automated pipelines immediately with no dependency on the interactive UX work. Smallest deliverable that adds real value.

**Independent Test**: Set `ENVY_PASSPHRASE_PRODUCTION` in the environment, run `envy encrypt`, assert exit 0 and that `envy.enc` contains the production envelope. No TTY required.

**Acceptance Scenarios**:

1. **Given** `ENVY_PASSPHRASE_PRODUCTION=secret` is set and the vault has a `production` environment, **When** `envy encrypt` is run without a TTY, **Then** `envy.enc` is written with the `production` envelope sealed and the process exits 0.
2. **Given** only `ENVY_PASSPHRASE=fallback` is set and the vault has `development` and `staging`, **When** `envy encrypt` is run, **Then** both environments are sealed using `fallback` as the passphrase.
3. **Given** `ENVY_PASSPHRASE_STAGING=s1` and `ENVY_PASSPHRASE=fallback` are both set and the vault has `development` and `staging`, **When** `envy encrypt` is run, **Then** `staging` is sealed with `s1` and `development` is sealed with `fallback`.
4. **Given** no passphrase env var is set and no TTY is available, **When** `envy encrypt` is run, **Then** the process exits non-zero with a clear error message.

---

### User Story 2 — Smart Merge with Atomic Writes (Priority: P2)

A developer encrypts only the `development` environment while a teammate has already encrypted `production`. The existing `envy.enc` contains the `production` envelope. After running `envy encrypt -e development`, the file must contain both envelopes: the newly sealed `development` and the untouched `production`.

**Why this priority**: Prevents accidental data loss during team collaboration. Without this, every encrypt overwrites all other environments' sealed data.

**Independent Test**: Create `envy.enc` with a `production` envelope, encrypt `development` only, assert the resulting `envy.enc` contains both envelopes with `production` unchanged.

**Acceptance Scenarios**:

1. **Given** `envy.enc` exists with a `production` envelope, **When** `envy encrypt -e development` is run, **Then** `envy.enc` contains both `production` (unchanged) and the newly sealed `development`.
2. **Given** `envy.enc` does not exist, **When** `envy encrypt` seals two environments, **Then** a new `envy.enc` is created with both envelopes.
3. **Given** a crash occurs mid-write, **When** the disk is inspected, **Then** the previous `envy.enc` is intact (atomic write guarantee — no partial or empty file).
4. **Given** `envy.enc` already contains a `development` envelope, **When** `envy encrypt -e development` is run again, **Then** only the `development` envelope is replaced; all other envelopes are byte-for-byte identical to before.

---

### User Story 3 — Interactive Multi-Environment Selection (Priority: P3)

A developer runs `envy encrypt` with a TTY and no environment flag. A checkbox menu lists all environments in the local vault. The developer selects a subset and enters passphrases; the artifact is updated for the selected environments only.

**Why this priority**: Quality-of-life for interactive users; relies on headless (P1) and smart merge (P2) foundations.

**Independent Test**: Run `envy encrypt` interactively, simulate selecting two of three available environments, assert only the selected two are updated in `envy.enc`.

**Acceptance Scenarios**:

1. **Given** the vault contains `development`, `staging`, and `production`, **When** `envy encrypt` is run interactively and the user selects `development` and `staging`, **Then** only those two envelopes are updated in `envy.enc`.
2. **Given** the user selects zero environments (deselects all) and confirms, **Then** the process exits 0 without modifying `envy.enc` and prints "Nothing to encrypt."
3. **Given** the vault is empty (no environments defined), **When** `envy encrypt` is run interactively, **Then** the process exits with the message "No environments found. Use `envy set` to add secrets first."

---

### User Story 4 — Key-Rotation Protection (Pre-flight Check) (Priority: P4)

A developer is about to re-encrypt an environment that already exists in `envy.enc`. If the passphrase they enter does not match the existing sealed data, the system warns them that they are about to rotate the key and requires explicit confirmation before proceeding.

**Why this priority**: Safety net for an irreversible operation. Depends on US2 (smart merge) and US3 (interactive selection).

**Independent Test**: Seal `development` with passphrase A, attempt to re-encrypt with passphrase B, assert rotation warning is displayed and operation is aborted unless explicitly confirmed with `y`.

**Acceptance Scenarios**:

1. **Given** `envy.enc` has a `development` envelope sealed with `passphrase-A`, **When** the user encrypts `development` with `passphrase-B`, **Then** a warning is displayed: "Passphrase does not match existing data. Continuing will ROTATE the key. Are you sure? (y/N)".
2. **Given** the rotation warning is shown, **When** the user types `N` or presses Enter, **Then** the envelope is NOT updated and the process exits 0.
3. **Given** the rotation warning is shown, **When** the user types `y`, **Then** the envelope IS updated with the new passphrase and the process exits 0.
4. **Given** an environment has no existing envelope in `envy.enc` (new environment), **When** the user enters a passphrase interactively, **Then** the passphrase must be entered twice to confirm (no rotation warning).

---

### User Story 5 — Diceware Passphrase Suggestion (Priority: P5)

When encrypting a new environment interactively (no existing envelope), the system suggests a strong randomly generated passphrase. If the user accepts by pressing Enter, a prominent "SAVE THIS NOW" banner displays the passphrase before encrypting.

**Why this priority**: Improves security hygiene for new users. Purely additive with no impact on correctness.

**Independent Test**: Encrypt a new environment interactively, accept the suggested passphrase, assert a "SAVE THIS NOW" banner with the passphrase is displayed before `envy.enc` is written.

**Acceptance Scenarios**:

1. **Given** a new environment with no existing envelope, **When** the interactive passphrase prompt appears, **Then** a suggested Diceware passphrase (4+ words) is displayed as the default option.
2. **Given** the suggestion is shown, **When** the user presses Enter without typing, **Then** the suggested passphrase is used and a high-visibility "SAVE THIS NOW" banner displays it before the artifact is written.
3. **Given** the suggestion is shown, **When** the user types their own passphrase instead, **Then** the user's passphrase is used with no banner, and the normal confirmation prompt applies.

---

### Edge Cases

- What happens when `envy.enc` exists but contains invalid JSON? → Process must abort with a clear parse error; must not silently overwrite.
- What if `ENVY_PASSPHRASE_<ENV>` is set for an environment not present in the local vault? → The variable is ignored; only environments present in the vault are encrypted.
- What if an environment name contains hyphens or mixed case (e.g. `my-env`)? → Normalised to `ENVY_PASSPHRASE_MY_ENV` (uppercase, hyphens → underscores).
- What if `envy.enc.tmp` already exists from a previous crashed write? → Overwritten silently before the new write begins.
- What if the filesystem does not support atomic rename (e.g., cross-device move)? → Operation fails with a clear error; no partial file is left.
- What if an environment is deleted from the vault between selection and encryption? → Skip with a warning; do not abort the entire operation.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When `ENVY_PASSPHRASE_<ENV>` is set (where `<ENV>` is the uppercase, normalised environment name), the system MUST use that value as the passphrase for that environment without prompting.
- **FR-002**: When `ENVY_PASSPHRASE` is set and no environment-specific variable exists for an environment, the system MUST use `ENVY_PASSPHRASE` as the fallback passphrase for that environment without prompting.
- **FR-003**: The system MUST NOT support a `--passphrase` CLI flag; passphrases MUST only come from environment variables (headless) or interactive prompts (TTY).
- **FR-004**: When running headlessly (a passphrase env var is resolvable), the system MUST bypass the interactive selection menu, diceware suggestion, and pre-flight check, and encrypt all vault environments that have a resolvable passphrase.
- **FR-005**: The system MUST read the existing `envy.enc` before writing and MUST preserve all envelopes not being updated in the output (smart merge).
- **FR-006**: The system MUST write `envy.enc` atomically: first write to `envy.enc.tmp`, then rename to `envy.enc`. A crash between write and rename MUST leave the previous `envy.enc` intact.
- **FR-007**: When running interactively with no `-e` flag, the system MUST present a checkbox selection of all environments in the local vault.
- **FR-008**: When an environment already has an envelope in `envy.enc` and the user is encrypting interactively, the system MUST attempt to decrypt the existing envelope with the provided passphrase. If decryption fails, the system MUST display the key-rotation warning and default to aborting (require explicit `y` to proceed).
- **FR-009**: When encrypting a brand-new environment (no existing envelope) interactively, the system MUST require the passphrase to be entered twice for confirmation.
- **FR-010**: When encrypting a brand-new environment interactively, the system MUST suggest a Diceware passphrase (minimum 4 words, cryptographically random). If the user accepts, the system MUST display a high-visibility "SAVE THIS NOW" banner showing the accepted passphrase before writing.
- **FR-011**: When the user selects zero environments from the interactive menu, the system MUST exit 0 without modifying `envy.enc` and display "Nothing to encrypt."
- **FR-012**: Environment names MUST be normalised to uppercase with hyphens replaced by underscores when constructing the `ENVY_PASSPHRASE_<ENV>` lookup key (e.g., `my-env` → `ENVY_PASSPHRASE_MY_ENV`).
- **FR-013**: If `envy.enc` exists and cannot be parsed as valid JSON, the system MUST abort with a clear error before attempting any encryption.

### Key Entities

- **Envelope**: A single environment's sealed payload within `envy.enc`. Contains encrypted secret data and the metadata needed to decrypt it. One envelope per environment; the artifact holds one or many envelopes keyed by environment name.
- **Passphrase**: A secret string used to seal and unseal an Envelope. Provided via environment variable (headless) or interactive prompt. Never stored; used only transiently.
- **Artifact (`envy.enc`)**: The versioned, committable file containing all sealed Envelopes. Must remain valid JSON at all times. All mutations are atomic.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A CI pipeline can seal a `production` environment using only an environment variable — zero interactive prompts, exits 0 within 5 seconds on standard hardware.
- **SC-002**: Re-encrypting a single environment in an artifact containing 10 environments leaves all other 9 envelopes byte-for-byte identical.
- **SC-003**: A process killed mid-write leaves the previous `envy.enc` intact and readable 100% of the time (atomic write guarantee).
- **SC-004**: When a passphrase mismatch is detected, 100% of attempts to proceed without explicit `y` confirmation are aborted.
- **SC-005**: A developer encrypting a new environment interactively sees a Diceware passphrase suggestion on 100% of new-environment encrypt operations.
- **SC-006**: Environment variable normalisation correctly resolves any environment name (including hyphens, mixed case) to the correct `ENVY_PASSPHRASE_*` key on all supported platforms (Linux, macOS, Windows).

## Assumptions

- The vault layer already exposes a method to list all environment names for a project (used by the interactive selection menu).
- `envy.enc` uses a stable top-level JSON object keyed by environment name; this structure is not being changed by this feature.
- The Diceware word list (EFF large wordlist, 7776 words) is embedded at compile time; no network access is required.
- Headless mode encrypts all vault environments that have a resolvable passphrase. To encrypt only one environment headlessly, the operator sets only `ENVY_PASSPHRASE_<ENV>` for that environment and ensures `ENVY_PASSPHRASE` is not set.
- The pre-flight decryption check uses the same passphrase the user just provided — no separate "old passphrase" input is required.

## Out of Scope

- Changing the encryption algorithm or KDF parameters.
- Adding a `--passphrase` CLI flag (explicitly excluded by FR-003).
- Decrypting `envy.enc` (handled by `envy decrypt`; not modified by this feature).
- Rotating passphrases in bulk across all environments in a single command.
- Sharing or distributing passphrases through any built-in mechanism.
