# Feature Specification: Pre-Encrypt Secret Diff

**Feature Branch**: `011-envy-diff`
**Created**: 2026-03-28
**Status**: Draft
**Input**: User description: "envy diff — compare decrypted local vault secrets against the currently sealed envy.enc artifact for a specific environment, showing Git-like additions, deletions, and modifications before running envy encrypt."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Review Changes Before Encrypting (Priority: P1)

A developer has been modifying secrets in their local vault (adding new keys, updating values, removing stale entries) and wants to verify exactly what has changed before running `envy encrypt`. They run `envy diff` to see a clear, Git-style summary of additions, deletions, and modifications for the default `development` environment — without exposing any actual secret values in their terminal.

**Why this priority**: This is the core use case. Every user who sees "Modified" in `envy status` today has no way to verify *what* changed before sealing. This eliminates the trust gap between `envy status` and `envy encrypt`, which is the single biggest workflow blind spot.

**Independent Test**: Create a vault with secrets, seal an artifact, then add one new key, modify one existing key, and delete one existing key. Run `envy diff`. The output must show exactly three lines: one addition (+), one modification (~), and one deletion (-) — with only key names visible, no values.

**Acceptance Scenarios**:

1. **Given** a vault with `development` containing keys `A`, `B`, `C` sealed into `envy.enc`, and the user then adds key `D`, modifies the value of `B`, and deletes `C`, **When** the user runs `envy diff`, **Then** the output shows `+ D` (green), `~ B` (yellow), `- C` (red), sorted alphabetically by key name, and no secret values are printed.
2. **Given** a vault with `development` sealed into `envy.enc` and no changes since, **When** the user runs `envy diff`, **Then** the output prints "No differences." and the command exits with code 0.
3. **Given** a vault with `development` sealed into `envy.enc`, **When** the user runs `envy diff` and enters the correct passphrase, **Then** the diff output appears. If the user enters an incorrect passphrase, the command exits with an authentication error message and a non-zero exit code.
4. **Given** a vault where `development` has been modified since the last seal, **When** the user runs `envy diff`, **Then** the command exits with code 1 (differences found), allowing scripts to detect drift.

---

### User Story 2 — Reveal Secret Values for Detailed Inspection (Priority: P2)

A developer needs to see the actual *before* and *after* values for a modified secret (e.g., to verify they updated a connection string correctly). They use the `--reveal` flag to see full plaintext values alongside the key names.

**Why this priority**: Power users and incident responders need value-level visibility. The safe default (keys-only) covers 90% of use cases, but the remaining 10% — debugging a bad rotation, auditing a suspect change — require seeing the actual data. This must be explicitly opt-in.

**Independent Test**: Modify one secret value in the vault, then run `envy diff --reveal`. The output must show the key name, the old value (from the artifact), and the new value (from the vault).

**Acceptance Scenarios**:

1. **Given** key `DATABASE_URL` has value `postgres://old` in `envy.enc` and `postgres://new` in the vault, **When** the user runs `envy diff --reveal`, **Then** the output shows both the old and new values alongside the key name.
2. **Given** a new key `STRIPE_KEY` exists in the vault but not in `envy.enc`, **When** the user runs `envy diff --reveal`, **Then** the output shows the new value for `STRIPE_KEY` and marks the old value as absent.
3. **Given** a key `OLD_TOKEN` exists in `envy.enc` but has been deleted from the vault, **When** the user runs `envy diff --reveal`, **Then** the output shows the old value from the artifact and marks the new value as absent.
4. **Given** the user runs `envy diff` *without* `--reveal`, **Then** no secret values appear anywhere in stdout or stderr, regardless of change type.

---

### User Story 3 — Machine-Readable Diff for CI/CD Pipelines (Priority: P3)

A CI/CD pipeline needs to programmatically detect exactly what changed between the vault and `envy.enc` — not just "something changed" (which `envy status` already provides), but a structured list of additions, deletions, and modifications with key names and change types that can be parsed by automation tooling.

**Why this priority**: Teams using `envy status --format json` as a CI gate already know *that* something drifted. This gives them the ability to answer *what* drifted in the same pipeline, enabling automated Slack notifications, audit logs, and approval workflows.

**Independent Test**: Run `envy diff --format json` and pipe to a JSON parser. The output must be valid JSON containing an array of change entries, each with a key name and change type. When `--reveal` is also set, values are included.

**Acceptance Scenarios**:

1. **Given** a vault with changes, **When** the user runs `envy diff --format json`, **Then** stdout is valid JSON containing a list of change entries with `key` and `type` fields (`"added"`, `"removed"`, `"modified"`), and no `old_value`/`new_value` fields are present.
2. **Given** a vault with changes, **When** the user runs `envy diff --format json --reveal`, **Then** each change entry includes `old_value` and `new_value` fields (null when not applicable).
3. **Given** no differences exist, **When** the user runs `envy diff --format json`, **Then** the output is valid JSON with an empty changes array and the exit code is 0.
4. **Given** the exit code is 1 (differences found), **When** the JSON is parsed, **Then** the changes array is non-empty.

---

### User Story 4 — Diff Against a Missing Artifact (Priority: P4)

A developer has set up a project and added secrets but has never run `envy encrypt`. They run `envy diff` to preview what *would* be sealed. Since there is no `envy.enc` to compare against, every vault secret is reported as an addition.

**Why this priority**: This covers the first-time encryption workflow. Instead of blindly running `envy encrypt` and hoping everything looks right, the developer can preview what will go into the artifact. Lower priority because it's a one-time event per project.

**Independent Test**: Create a vault with secrets in `development` but no `envy.enc` file. Run `envy diff`. All keys appear as additions.

**Acceptance Scenarios**:

1. **Given** a vault with keys `A`, `B`, `C` in `development` and no `envy.enc` file exists, **When** the user runs `envy diff`, **Then** the output shows `+ A`, `+ B`, `+ C` (all additions) and prints a notice that no artifact was found.
2. **Given** no `envy.enc` and no secrets in the vault, **When** the user runs `envy diff`, **Then** the output prints "No differences." and exits with code 0.
3. **Given** no `envy.enc` file, **When** the user runs `envy diff`, **Then** no passphrase is prompted (there is nothing to decrypt).

---

### User Story 5 — Diff a Non-Default Environment (Priority: P5)

A developer wants to check what changed in `staging` or `production` before encrypting. They use the `-e` flag to target a specific environment.

**Why this priority**: Multi-environment support is table stakes for team workflows, but the default `development` environment covers the majority of individual developer usage. This extends the P1 story to all environments.

**Independent Test**: Create secrets in `staging`, seal, modify one, then run `envy diff -e staging`. The modification appears in the output.

**Acceptance Scenarios**:

1. **Given** a vault with `staging` secrets sealed into `envy.enc`, and the user modifies one staging secret, **When** the user runs `envy diff -e staging`, **Then** only staging changes are shown.
2. **Given** `envy.enc` contains `production` and `staging`, but the vault has no `production` environment, **When** the user runs `envy diff -e production`, **Then** all secrets from the artifact's `production` envelope appear as deletions (present in artifact, absent from vault).
3. **Given** `envy.enc` does not contain a `testing` envelope but the vault has `testing` secrets, **When** the user runs `envy diff -e testing`, **Then** all vault secrets appear as additions (not in artifact, present in vault), and no passphrase is prompted for an absent envelope.

---

### Edge Cases

- What happens when `envy.enc` exists but is malformed JSON? The command exits with a clear error message ("Artifact unreadable") and a non-zero exit code.
- What happens when the environment exists in neither the vault nor the artifact? The command exits with an error: "Environment 'foo' not found in vault or artifact."
- What happens when the passphrase is wrong? The command exits with an authentication error and a non-zero exit code. No partial diff is shown.
- What happens with per-environment passphrases? The diff reads the passphrase for the specific environment being diffed, following the same resolution order as `envy encrypt`: `ENVY_PASSPHRASE_<ENV>` → `ENVY_PASSPHRASE` → interactive prompt.
- What happens when a key exists in both sides with identical values? It is excluded from the diff output entirely (no change = no line).
- What happens with very large environments (1,000+ keys)? The diff renders fully without truncation. Key-only mode is fast because no value comparison is technically needed — but value comparison is required to detect modifications, so all values are decrypted in memory but never printed unless `--reveal` is set.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `envy diff` command MUST compare secrets in the local vault against secrets sealed in `envy.enc` for a single environment, reporting additions, deletions, and modifications.
- **FR-002**: The command MUST default to the `development` environment when `-e` is not provided, consistent with the project's progressive disclosure convention.
- **FR-003**: The command MUST NOT print decrypted secret values to stdout or stderr by default. Only key names and change indicators (`+`, `-`, `~`) are permitted in the default output.
- **FR-004**: The command MUST support a `--reveal` flag that includes plaintext secret values (old and new) in the output for all change types.
- **FR-005**: The command MUST use colored output in the default (table/text) format: green for additions, red for deletions, yellow for modifications. Color MUST be suppressed when stdout is not a TTY or when `NO_COLOR` is set.
- **FR-006**: The command MUST support `--format json` for machine-readable output. The JSON MUST contain: the environment name, a list of changes (each with `key`, `type`), and a summary count. When `--reveal` is active, each change entry MUST also include `old_value` and `new_value` (null where not applicable).
- **FR-007**: The command MUST exit with code 0 when no differences are found, code 1 when differences are found, and code 2 or higher for errors (authentication failure, missing vault, malformed artifact).
- **FR-008**: The command MUST resolve the artifact passphrase using the same priority order as `envy encrypt`: environment-specific env var (`ENVY_PASSPHRASE_<ENV>`) → global env var (`ENVY_PASSPHRASE`) → interactive TTY prompt.
- **FR-009**: When `envy.enc` does not exist, the command MUST treat all vault secrets as additions (no passphrase prompt needed) and print a notice that no artifact was found.
- **FR-010**: When the target environment does not exist in `envy.enc` but exists in the vault, the command MUST treat all vault secrets as additions (no passphrase prompt needed for an absent envelope).
- **FR-011**: When the target environment exists in `envy.enc` but does not exist in the vault, the command MUST unseal the envelope and treat all artifact secrets as deletions.
- **FR-012**: Diff entries MUST be sorted alphabetically by key name.
- **FR-013**: The diff computation MUST occur in the Core layer. The CLI layer is responsible only for rendering. The Core layer returns a structured diff result; it MUST NOT perform any I/O or formatting.
- **FR-014**: All decrypted secret values used during diff computation MUST be held in memory-zeroed containers and dropped as soon as the comparison is complete. Values MUST NOT be logged, cached, or written to disk.
- **FR-015**: When `--reveal` is used, the command MUST print a one-line security warning to stderr before the diff output (e.g., "Warning: secret values are visible in the output below.").

### Key Entities

- **Diff Entry**: A single key-level change between the vault and the artifact. Contains: key name, change type (added, removed, modified), optional old value (from artifact), optional new value (from vault).
- **Diff Report**: The complete comparison result for one environment. Contains: environment name, a sorted list of diff entries, and summary counts (additions, removals, modifications).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can review the exact set of secret key changes before running `envy encrypt` in under 5 seconds for environments with up to 500 secrets.
- **SC-002**: 100% of changes between the vault and the artifact are correctly classified — no additions misclassified as modifications, no deletions missed, no false positives.
- **SC-003**: By default (without `--reveal`), zero secret values appear in stdout or stderr output across all output formats and all change types.
- **SC-004**: The JSON output passes validation by standard JSON parsers in 100% of runs, with no post-processing required.
- **SC-005**: The exit code correctly distinguishes "no differences" (0), "differences found" (1), and "error" (2+) in 100% of runs, enabling reliable scripted quality gates.
- **SC-006**: The `--reveal` flag is the sole mechanism for value visibility — removing it from a command invocation guarantees no values are exposed, with no configuration, environment variable, or other override that could leak values silently.

---

## Assumptions

- The `envy diff` command operates on a single environment per invocation. Diffing all environments at once is out of scope for this feature.
- The artifact passphrase resolution follows the identical logic already implemented for `envy encrypt` and `envy decrypt` — no new passphrase discovery mechanisms are introduced.
- Value comparison uses byte-level equality on the decrypted plaintext. No semantic diffing (e.g., treating `true` and `True` as equal) is performed.
- The `--reveal` flag has no persistent state — it must be explicitly passed on every invocation. There is no configuration option to make reveal the default.
- Color output follows the `NO_COLOR` convention (https://no-color.org/) and automatic TTY detection. No `--color` flag is introduced; this matches the existing CLI behavior.
- The `envy diff` command does not modify the vault, the artifact, or the sync markers. It is strictly read-only.
- When `envy.enc` is missing or the target environment is absent from the artifact, the diff is computed against an empty set — no passphrase is needed since there is nothing to decrypt.
