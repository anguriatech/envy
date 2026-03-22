# Feature Specification: CLI Sync Commands (encrypt / decrypt)

**Feature Branch**: `006-cli-sync-commands`
**Created**: 2026-03-22
**Status**: Draft
**Input**: User description: "We are starting the specification for `006-cli-sync-commands`. The goal of this module is to provide the user interface using `clap` for the new GitOps synchronization commands. It will parse user input, securely prompt for passphrases (or read them from the environment for CI/CD), call the core sync functions, and format the output beautifully for the developer."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Seal vault into a shareable artifact (Priority: P1)

A developer has added secrets to their local vault and wants to share them with their team via Git. They run `envy encrypt` (or `envy enc`), enter a shared team passphrase twice (to catch typos), and a file called `envy.enc` is written to the project directory. They commit and push this file. Teammates can now run `envy decrypt` to restore the secrets after pulling.

**Why this priority**: This is the first half of the GitOps workflow. Without it, secrets cannot be shared at all. Every other sync feature depends on a valid `envy.enc` existing.

**Independent Test**: Run `envy encrypt` in a project with at least one secret. Confirm `envy.enc` is created in the current directory and contains no plaintext values.

**Acceptance Scenarios**:

1. **Given** a project with secrets in the vault, **When** the developer runs `envy encrypt` and enters a passphrase twice correctly, **Then** `envy.enc` is written to the current directory and the terminal shows a confirmation of which environments were sealed.
2. **Given** the passphrase entries do not match, **When** the developer runs `envy encrypt`, **Then** the tool rejects the input with a clear error message and does not write any file.
3. **Given** the developer runs `envy encrypt` again after a previous run, **When** the passphrase is entered correctly, **Then** the existing `envy.enc` is safely overwritten and the operation succeeds.
4. **Given** a project with no secrets, **When** the developer runs `envy encrypt`, **Then** an `envy.enc` with an empty environments map is produced and the tool reports zero environments sealed.
5. **Given** a project with multiple environments, **When** the developer runs `envy enc -e staging`, **Then** only the `staging` environment is sealed into `envy.enc`.

---

### User Story 2 — Restore secrets from a shared artifact (Priority: P1)

A developer pulls the latest `envy.enc` from the repository after a teammate updated secrets. They run `envy decrypt` (or `envy dec`), enter the shared team passphrase, and the tool imports all decrypted secrets into the local vault. The developer's local environment is now in sync.

**Why this priority**: Without decrypt, the artifact is write-only and no sharing benefit is realised. These two commands form an atomic pair.

**Independent Test**: Seal an artifact with a known passphrase and a known set of secrets, then decrypt it and verify the vault contains the expected values.

**Acceptance Scenarios**:

1. **Given** a valid `envy.enc` with one or more environments, **When** the developer runs `envy decrypt` with the correct passphrase, **Then** all secrets are upserted into the vault and the terminal shows a green success message per imported environment.
2. **Given** a valid `envy.enc`, **When** the developer runs `envy decrypt` with a wrong passphrase, **Then** all environments are skipped, the tool exits with a non-zero code, and the vault is untouched.
3. **Given** no `envy.enc` in the current directory, **When** the developer runs `envy decrypt`, **Then** the tool exits with a clear "file not found" message and a non-zero exit code.
4. **Given** a corrupted or non-JSON `envy.enc`, **When** the developer runs `envy decrypt`, **Then** the tool exits immediately with a "malformed artifact" error and a non-zero exit code.

---

### User Story 3 — Progressive Disclosure: partial-key team member (Priority: P2)

An enterprise team uses separate passphrases per environment: a shared dev key for `development` and `staging`, and a restricted prod key only for `production`. A developer runs `envy decrypt` with the dev key. The tool imports the two development environments and shows a non-failing informational note that `production` was skipped — the developer is not locked out and receives no alarm.

**Why this priority**: Without this UX handling, developers with partial access would see a failure or receive no output for their accessible environments, breaking the Progressive Disclosure model entirely.

**Independent Test**: Create an artifact where two environments use different passphrases. Decrypt with one of them and confirm the correct environment is imported while the other is listed as skipped — no error exit code.

**Acceptance Scenarios**:

1. **Given** an artifact with `development` (dev key) and `production` (prod key), **When** the developer runs `envy decrypt` with the dev key, **Then** `development` secrets are imported, `production` is reported as skipped in a yellow/dim informational line, and the command exits with code 0.
2. **Given** some environments are imported and some are skipped, **When** the operation completes, **Then** the output clearly separates "imported" (green) from "skipped" (yellow/dim) without mixing them.

---

### User Story 4 — Headless CI/CD decryption (Priority: P2)

A CI/CD pipeline (e.g., GitHub Actions) needs to decrypt `envy.enc` without a human at a terminal. The pipeline sets `ENVY_PASSPHRASE` as an environment variable containing the team passphrase. When `envy decrypt` is invoked, it detects the environment variable, skips the interactive prompt, and proceeds automatically.

**Why this priority**: Without headless support, teams cannot use Envy in automated pipelines — a critical blocker for Phase 2 adoption goals.

**Independent Test**: Run `envy decrypt` with `ENVY_PASSPHRASE` set in the environment and stdin redirected from `/dev/null`. Confirm the command succeeds without hanging.

**Acceptance Scenarios**:

1. **Given** `ENVY_PASSPHRASE` is set in the environment, **When** `envy decrypt` is run, **Then** the passphrase is read from the environment variable, no terminal prompt is shown, and the operation completes normally.
2. **Given** `ENVY_PASSPHRASE` is set in the environment, **When** `envy encrypt` is run, **Then** the passphrase is read from the environment variable, no double-entry confirmation is required, and `envy.enc` is written.
3. **Given** `ENVY_PASSPHRASE` is set but is empty or whitespace, **When** either command is run, **Then** the environment variable is treated as unset and the interactive prompt is shown (or an error is raised if stdin is not a terminal).

---

### Edge Cases

- What happens when the passphrase confirmation entries do not match during `envy encrypt`? → Clear error, no file written.
- What happens when `ENVY_PASSPHRASE` is set to whitespace? → Treated as unset; fall back to interactive prompt.
- What happens when `envy decrypt` is run and zero environments can be decrypted? → Exit code 1, clear "nothing imported — check your passphrase" message.
- What happens when the vault has no environments yet and `envy encrypt` is run? → Succeeds, writes an artifact with an empty environments map, reports 0 environments sealed.
- What happens when `envy.enc` is present but has an unsupported schema version? → Tool exits with a "unsupported version" error and upgrade instructions.
- What happens when the process is run in a directory without `envy.toml`? → Inherits the existing "manifest not found" error from the CLI dispatch layer (no special handling needed).

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide an `encrypt` command (alias `enc`) that reads all secrets from the local vault and writes them into an `envy.enc` artifact in the current directory.
- **FR-002**: The system MUST provide a `decrypt` command (alias `dec`) that reads `envy.enc` from the current directory, unseals it, and upserts all successfully decrypted secrets into the local vault.
- **FR-003**: In interactive mode, `encrypt` MUST prompt the user for a passphrase twice and reject mismatched entries without writing any file.
- **FR-004**: In interactive mode, `decrypt` MUST prompt the user for a passphrase once.
- **FR-005**: Passphrase input MUST be hidden in the terminal (no character echo) in interactive mode.
- **FR-006**: Both commands MUST check for the `ENVY_PASSPHRASE` environment variable before prompting; if present and non-empty, it is used as the passphrase without any terminal interaction.
- **FR-007**: `encrypt` MUST support an optional `-e` / `--env` flag to seal only the named environment(s) into the artifact.
- **FR-008**: On successful `decrypt`, the terminal MUST display a green success line for each imported environment listing the count of secrets upserted.
- **FR-009**: On successful `decrypt`, any skipped environments MUST be shown as a non-failing informational message (yellow or dim styling) — the command MUST NOT exit with a non-zero code solely because environments were skipped.
- **FR-010**: If `decrypt` completes with zero imported environments, the command MUST exit with a non-zero code and print a "nothing imported — check your passphrase" message.
- **FR-011**: On successful `encrypt`, the terminal MUST display a confirmation message listing the environments sealed and the path of the written artifact.
- **FR-012**: Both commands MUST look for (and write) `envy.enc` in the current working directory.
- **FR-013**: `encrypt` MUST overwrite any existing `envy.enc` file without prompting for confirmation.
- **FR-014**: Both commands MUST integrate with the existing manifest and vault lifecycle (require `envy.toml`, use the OS-managed master key to open the vault).
- **FR-015**: Both commands MUST exit with a non-zero code and a descriptive error message on any hard failure (file not found, malformed artifact, passphrase empty, vault error).

### Key Entities

- **Passphrase**: The secret string used to seal/unseal the artifact. Never persisted to disk. Hidden during input. May originate from interactive terminal prompt or `ENVY_PASSPHRASE` environment variable.
- **Sync Artifact (`envy.enc`)**: The encrypted file produced by `encrypt` and consumed by `decrypt`. Located in the project root (same directory as `envy.toml`). Safe to commit to version control.
- **Import Result**: The outcome of a `decrypt` operation — a list of successfully imported environments (with secret counts) and a list of skipped environments.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can seal and share their entire vault in under 15 seconds from running `envy encrypt` to having a committed `envy.enc`.
- **SC-002**: A teammate can restore all shared secrets in under 15 seconds from running `envy decrypt` with the correct passphrase.
- **SC-003**: A CI/CD pipeline can decrypt secrets without any terminal interaction when `ENVY_PASSPHRASE` is set as a pipeline secret.
- **SC-004**: When a developer with partial access runs `envy decrypt`, they receive 100% of their accessible environments and a clear, non-alarming message about inaccessible ones — no support ticket needed.
- **SC-005**: The passphrase is never visible in the terminal output at any point during the encrypt or decrypt flow.
- **SC-006**: Every error scenario (file not found, wrong passphrase, malformed artifact, passphrase mismatch) produces a message that allows the developer to self-diagnose and retry without reading documentation.

---

## Dependencies

- Feature `005-gitops-sync-artifact` MUST be fully implemented before this feature can begin. The CLI layer calls `seal_artifact`, `unseal_artifact`, `write_artifact`, and `read_artifact` from `crate::core::sync`.
- The existing CLI dispatch mechanism (`src/cli/mod.rs`) and manifest/vault lifecycle must remain unchanged. This feature extends `Commands` with two new variants.

## Assumptions

- The artifact is always written to and read from the current working directory (same directory as `envy.toml`). A `--output` / `--path` flag is out of scope for this feature.
- The passphrase confirmation prompt during `envy encrypt` is a single re-entry (not repeated on mismatch); a mismatch exits with an error requiring the user to re-run the command.
- Coloured terminal output uses ANSI escape codes and is suppressed automatically when stdout is not a TTY (pipe or file redirect).
- The `ENVY_PASSPHRASE` environment variable name is the canonical name for CI/CD headless mode (distinct from the vault master key, which is managed by the OS keyring).
- Sealing with `encrypt` when the vault has no environments is not an error — it produces a valid but empty artifact.
- The `-e` flag on `encrypt` accepts a single environment name per invocation (multi-env selection via repeated flag is out of scope).
