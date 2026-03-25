# Feature Specification: Vault Sync Status Command

**Feature Branch**: `010-status-command`
**Created**: 2026-03-25
**Status**: Draft
**Input**: User description: "Create a technical specification for the new `envy status` command."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Instant Sync Awareness Before Encrypting (Priority: P1)

A developer runs `envy status` before sharing secrets with teammates. They want to know immediately, without typing a passphrase, which environments are out of date in `envy.enc` so they know what needs to be encrypted next.

**Why this priority**: The most common pain point — developers share an `envy.enc` file via Git, make local secret changes, then forget to re-encrypt before committing. This command eliminates that gap. It requires no passphrase and no network access, making it always safe to run.

**Independent Test**: Run `envy status` on a vault with three environments where one has been sealed, one has new secrets added after sealing, and one has never been sealed. The output must correctly label each as "In Sync", "Modified", and "Never Sealed" respectively.

**Acceptance Scenarios**:

1. **Given** a vault with a `production` environment sealed 10 minutes ago and no changes since, **When** the user runs `envy status`, **Then** `production` shows status "In Sync" and a relative time like "10 minutes ago".
2. **Given** a vault with a `staging` environment sealed yesterday but with a new secret added today, **When** the user runs `envy status`, **Then** `staging` shows status "Modified" with the modification time of the newest change.
3. **Given** a vault with a `development` environment that has never been encrypted, **When** the user runs `envy status`, **Then** `development` shows status "Never Sealed" and the Last Modified time reflects the most recently set secret.
4. **Given** a vault with no environments, **When** the user runs `envy status`, **Then** the command exits successfully with the message "No environments found."
5. **Given** no `envy.enc` artifact file exists, **When** the user runs `envy status`, **Then** the vault table still renders and the artifact section shows "No artifact found."

---

### User Story 2 — Sync State Stays Accurate After Encrypting (Priority: P2)

A developer runs `envy encrypt`, then immediately runs `envy status` and expects to see the newly encrypted environment marked as "In Sync" without any extra steps.

**Why this priority**: The sync status is only useful if it self-updates automatically. Without this wiring, the status would always show "Never Sealed" even after encrypting, making it misleading.

**Independent Test**: Seal `development`, then run `envy status`. The `development` row must show "In Sync" and a sealed timestamp matching the moment of encryption.

**Acceptance Scenarios**:

1. **Given** a `development` environment sealed via `envy encrypt`, **When** the user runs `envy status` immediately after, **Then** `development` shows "In Sync".
2. **Given** `development` was "In Sync", **When** the user adds a new secret with `envy set`, **Then** `envy status` shows `development` as "Modified".
3. **Given** `development` was "Modified", **When** the user runs `envy encrypt` again, **Then** `envy status` immediately shows `development` as "In Sync".

---

### User Story 3 — Machine-Readable Output for CI/CD Automation (Priority: P3)

A CI/CD pipeline checks whether all secrets are up to date before allowing a deployment to proceed. It runs `envy status --format json` and parses the output to detect any "Modified" or "Never Sealed" environments.

**Why this priority**: Automation use cases (CI gates, pre-commit hooks, deployment scripts) need structured output. This enables zero-human-intervention guardrails without relying on screen-scraping.

**Independent Test**: Run `envy status --format json` and pipe to a JSON parser. The output must be valid JSON containing an array of environment objects with fields: `name`, `secret_count`, `last_modified_at`, and `status`.

**Acceptance Scenarios**:

1. **Given** a vault with two environments, **When** the user runs `envy status --format json`, **Then** stdout is valid JSON and stderr is empty.
2. **Given** an environment with status "Modified", **When** the output is parsed, **Then** the `status` field contains the string `"modified"` (lowercase, machine-stable).
3. **Given** `envy.enc` does not exist, **When** the JSON is parsed, **Then** the `artifact` object has a `found` field of `false` and no error is thrown.

---

### User Story 4 — Artifact Metadata Visibility (Priority: P4)

A developer wants to know at a glance which environments are sealed in the shared `envy.enc` file and when it was last written, so they can confirm the file is current before distributing it.

**Why this priority**: The vault tracks local state; `envy.enc` is the shared artifact. Teams need to see both sides — local vault state and artifact state — without decrypting.

**Independent Test**: Run `envy status` when `envy.enc` contains two sealed environments. The output must list those environment names and the file's last-write timestamp without decrypting any payloads.

**Acceptance Scenarios**:

1. **Given** `envy.enc` contains `production` and `staging` envelopes, **When** the user runs `envy status`, **Then** the output lists both environments as present in the artifact along with the file's last modified time.
2. **Given** `envy.enc` is present, **When** the user runs `envy status`, **Then** no passphrase is requested and no decryption occurs.
3. **Given** `envy.enc` contains an environment that does not exist in the local vault, **When** the user runs `envy status`, **Then** that environment is listed in the artifact section but absent from the vault table.

---

### Edge Cases

- What happens when the vault has not been initialized? → The command fails with a clear "Vault not found" error before printing any output.
- What happens when `envy.enc` is malformed JSON? → The artifact section shows "Artifact unreadable" and the vault table still renders.
- What happens when an environment exists in `envy.enc` but not in the local vault? → It appears in the artifact section only, labelled as "artifact-only".
- What happens when an environment has secrets but they were all deleted after sealing? → The environment shows 0 secrets and "Modified" status (the vault changed after the last seal).
- What happens with a very large vault (100+ environments)? → The table renders fully without truncating rows.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The status command MUST display a table of all vault environments with columns: Environment name, number of secrets, time of last modification, and sync status.
- **FR-002**: Sync status MUST be one of three values: `In Sync` (sealed at or after the last change), `Modified` (changed since last seal), or `Never Sealed` (no seal record exists).
- **FR-003**: The status command MUST NEVER prompt for a passphrase or attempt to decrypt any secret payloads in `envy.enc`.
- **FR-004**: The status command MUST display artifact metadata: the list of environment names present in `envy.enc` and the file's last-write timestamp.
- **FR-005**: The status command MUST support a `--format json` flag that outputs structured data to stdout, suitable for machine parsing.
- **FR-006**: The JSON output MUST include for each environment: name, secret count, last modification timestamp (ISO 8601), and status string (`"in_sync"`, `"modified"`, or `"never_sealed"`).
- **FR-007**: The JSON output MUST include an `artifact` object with: `found` (boolean), `path` (string), `last_modified_at` (ISO 8601 or null), and `environments` (array of sealed environment names).
- **FR-008**: Every successful `envy encrypt` operation MUST record a seal timestamp for each environment it encrypts. This timestamp persists across restarts and is used to compute sync status.
- **FR-009**: When `envy.enc` does not exist, the artifact section MUST display a "not found" state without causing an error or non-zero exit.
- **FR-010**: When the vault contains no environments, the command MUST exit successfully with an informational message.
- **FR-011**: The "Last Modified" time in table output MUST be displayed as a human-readable relative time (e.g., "5 minutes ago", "2 days ago").
- **FR-012**: The status table MUST be sorted by environment name alphabetically.
- **FR-013**: Environments sealed by a version of `envy` before seal tracking was introduced MUST appear as "Never Sealed" until they are re-encrypted.

### Key Entities

- **Environment Status Record**: The computed sync state for one environment. Contains: environment name, secret count, most recent secret modification time, most recent seal time (if any), and derived sync status.
- **Sync Marker**: A persistent record that tracks when an environment was last successfully sealed. One record per environment; updated on each successful encrypt. Distinct from the encrypted payload — stores only a timestamp.
- **Artifact Metadata**: Non-secret information readable from `envy.enc` without decryption: the list of sealed environment names and the file's filesystem modification time.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The status command completes and prints output in under 500 milliseconds even with 100 environments and 1,000 total secrets, with no passphrase required.
- **SC-002**: Sync status accuracy is 100% — "In Sync" is never shown for an environment whose secrets were modified after its last seal.
- **SC-003**: JSON output passes validation by standard JSON parsers in 100% of runs, with no post-processing required.
- **SC-004**: After every successful `envy encrypt`, the sync status for all sealed environments reflects "In Sync" on the very next `envy status` call.
- **SC-005**: The command exits with code 0 in all non-error scenarios (vault missing, malformed artifact, and empty vault are each handled gracefully without a non-zero exit).

---

## Assumptions

- The `envy.enc` artifact filename and location are resolved using the same lookup logic as `envy encrypt` and `envy decrypt` (manifest or current directory).
- Relative time display (e.g., "5 minutes ago") uses the local system clock; no network time source is required.
- The `--format json` flag is the only output format variant in scope; `--format dotenv` and `--format shell` are out of scope for this feature.
- Seal timestamps have per-second resolution; sub-second accuracy is not required.
- The `status` command does not need to handle concurrent vault access differently from the existing vault concurrency model.
- Reading the list of environment names from `envy.enc` (without decrypting payloads) is safe and does not expose secret data.
