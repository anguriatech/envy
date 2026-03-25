# Feature Specification: Machine-Readable Output Formats

**Feature Branch**: `008-output-formats`
**Created**: 2026-03-24
**Status**: Implemented
**Input**: User description: "Adding machine-readable output formats via a global `--format` flag"

## QA Polish (2026-03-25)

The following items were addressed after the initial implementation:

1. **`export` uses `-e/--env` flag** — changed from a positional argument to a named flag (`-e/--env`) to match the rest of the CLI (`set`, `get`, `list`, `rm`, `run`, `migrate`, `encrypt`).
2. **`list` help text warns about value disclosure** — the `list` subcommand doc comment now explicitly states that non-table formats (`--format json|dotenv|shell`) decrypt and reveal secret values.
3. **E2E scenario added** — Scenario 5 in `tests/e2e_devops_scenarios.sh` covers `envy list --format json`, `envy export -e development --format shell`, and `envy export -e development` (default dotenv).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Script Consumes `envy list` Output (Priority: P1)

A DevOps engineer writes a shell script that reads secrets from the vault and injects them into a deployment pipeline. Today, parsing the human-readable table output is fragile and breaks when key names change width. With `--format json`, the script gets a stable, parseable payload every time.

**Why this priority**: This is the core unlock for CI/CD scripting. Without a machine-readable format, every downstream consumer is a fragile string-parser. JSON is the most universal interchange format.

**Independent Test**: Run `envy list --format json` against a populated vault and pipe the output to a JSON parser. Delivers standalone value: any script can now reliably read all secrets.

**Acceptance Scenarios**:

1. **Given** a vault with secrets `API_KEY` and `DB_HOST` in `development`, **When** the user runs `envy list --format json`, **Then** stdout is valid JSON containing both key-value pairs and nothing else.
2. **Given** a vault with no secrets in the target environment, **When** the user runs `envy list --format json`, **Then** stdout is `{}` or `[]` (empty but valid JSON).
3. **Given** the user runs `envy list` with no `--format` flag, **Then** the existing human-readable table output is unchanged (no regression).

---

### User Story 2 — Shell Script Sources Secrets Directly (Priority: P2)

A developer wants to load all secrets into the current shell session with a single command: `eval $(envy export --format shell)`. Each secret becomes a shell variable without any manual parsing. They also want to generate a `.env` file for Docker Compose via `envy export --format dotenv > .env`.

**Why this priority**: Directly enables the most common developer workflow — sourcing secrets into the shell — without requiring a wrapper script.

**Independent Test**: Run `envy export --format shell` and pipe the output to `eval`. All secrets must be available as environment variables in the current session.

**Acceptance Scenarios**:

1. **Given** a vault with `DB_PASS=s3cr3t` in `production`, **When** the user runs `envy export production --format shell`, **Then** stdout contains `export DB_PASS='s3cr3t'`.
2. **Given** a secret value containing a single quote (e.g., `it's here`), **When** output format is `shell`, **Then** the value is properly escaped so `eval` does not break.
3. **Given** the user runs `envy export production --format dotenv`, **Then** stdout contains `DB_PASS=s3cr3t` (one pair per line, no `export` prefix).
4. **Given** the user runs `envy export production` with no `--format` flag, **Then** the default format is `dotenv`.

---

### User Story 3 — VS Code Extension Reads a Single Secret Programmatically (Priority: P3)

The future VS Code extension calls `envy get KEY --format json` and parses the response without screen-scraping. This decouples the extension from any future changes to human-readable output wording.

**Why this priority**: Foundational for the Phase 3 VS Code Extension milestone. JSON output from `get` provides a stable, versioned contract between the CLI and GUI consumers.

**Independent Test**: Run `envy get API_KEY --format json` and verify the output is parseable JSON containing the key and its value.

**Acceptance Scenarios**:

1. **Given** `API_KEY=abc123` exists in the vault, **When** the user runs `envy get API_KEY --format json`, **Then** stdout is `{"key":"API_KEY","value":"abc123"}`.
2. **Given** the requested key does not exist, **When** `--format json` is used, **Then** stdout is `{"error":"key not found"}` and exit code is non-zero.

---

### Edge Cases

- What happens when `--format` receives an unrecognised value (e.g., `--format xml`)? → Immediate error with list of accepted values; exit code 2.
- What happens when a secret value contains newlines and the format is `dotenv`? → Value is quoted or escaped to remain single-line and parseable.
- What happens when `--format json` is used on a command that produces no data output (e.g., `envy set`)? → `--format` has no effect; the command proceeds normally with its standard confirmation message.
- What happens when stdout is redirected to a file and format is `table`? → `table` output goes to the file unchanged; no ANSI colour codes are emitted to non-TTY destinations.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CLI MUST accept a global `--format` flag (short form: `-f`) applicable to all commands.
- **FR-002**: The `--format` flag MUST accept exactly four values: `table`, `json`, `dotenv`, `shell`.
- **FR-003**: When `--format` is not specified, the default MUST be `table` for all commands except `export`.
- **FR-004**: The `list` command MUST support `--format json`, returning all key-value pairs as a JSON object.
- **FR-005**: The `get` command MUST support `--format json`, returning the key and its value as a JSON object; on key-not-found, a JSON error object MUST be returned with a non-zero exit code.
- **FR-006**: A new `export` command MUST be added; it MUST accept an environment name via the `-e/--env` named flag (default: `development`) and print all secrets for that environment to stdout.
- **FR-007**: The `export` command's default format MUST be `dotenv` when `--format` is not specified.
- **FR-008**: The `dotenv` format MUST produce one `KEY=value` line per secret with no additional decorators.
- **FR-009**: The `shell` format MUST produce one `export KEY='value'` line per secret; single quotes inside values MUST be escaped so the output is safe to `eval`.
- **FR-010**: The output layer MUST be separated from the data-retrieval layer so that adding a new format in future requires no changes to command logic.
- **FR-011**: All existing commands MUST continue to work identically when `--format` is omitted (zero regression).
- **FR-012**: When `--format` receives an invalid value, the CLI MUST exit with code 2 and print an error listing the accepted values.

### Key Entities

- **OutputFormat**: Represents the selected presentation mode (`table`, `json`, `dotenv`, `shell`). A presentation concern, not a data entity.
- **SecretRecord**: A key-value pair (plus optional environment label) that can be serialised into any output format.
- **ExportResult**: The ordered collection of SecretRecords produced by the `export` command for a given environment.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A shell script can parse `envy list --format json` output with a standard JSON tool without any string manipulation.
- **SC-002**: Running `eval $(envy export --format shell)` correctly populates all secrets as shell variables, including values containing special characters such as single quotes and spaces.
- **SC-003**: All existing command outputs are byte-for-byte identical when `--format` is omitted (zero regression verified by existing test suite).
- **SC-004**: Adding a fifth output format in future requires changes to at most one file outside the existing command handlers.
- **SC-005**: `envy get KEY --format json` returns parseable JSON for both the found and not-found cases, with an appropriate exit code in each.

## Scope

### In Scope

- Global `--format` / `-f` flag on all commands
- `json`, `dotenv`, and `shell` format implementations for read commands
- New `export` command (reads from local vault)
- Refactor of `list` and `get` output paths to use the new presentation layer
- Backward-compatible `table` default (existing output unchanged)

### Out of Scope

- CSV, XML, or TOML output formats
- Coloured / syntax-highlighted JSON
- Streaming or paginated output for large vaults
- Format selection via config file or environment variable (flag only)
- `export` reading from `envy.enc` (reads local vault only)

## Assumptions

- Secret values are valid UTF-8 strings; binary values are out of scope.
- The `table` format for `list` and `get` retains existing behaviour exactly, including any colour output when stdout is a TTY.
- `--format` on write commands (`set`, `rm`, `init`) is accepted by the parser but has no effect on output content.
- The `export` command reads from the local vault (not from `envy.enc`); artifact decryption is not part of this feature.
