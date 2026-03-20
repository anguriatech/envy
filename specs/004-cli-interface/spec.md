# Feature Specification: CLI Interface

**Feature Branch**: `004-cli-interface`
**Created**: 2026-03-19
**Status**: Draft

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Project Initialisation (Priority: P1)

A developer starts using Envy in a new project directory. They run a single command that creates both the `envy.toml` manifest (linking the directory to a vault project) and the vault project entry itself. From this moment on, any subdirectory within that project can be detected as belonging to Envy.

**Why this priority**: Every other command depends on a project being initialised. Without `init`, nothing else works. It is the entry point for all Envy usage.

**Independent Test**: Can be fully tested by running `envy init` in an empty directory and verifying that `envy.toml` is created with a valid `project_id`, and that subsequent CRUD commands succeed without error.

**Acceptance Scenarios**:

1. **Given** a directory with no `envy.toml`, **When** the user runs `envy init`, **Then** an `envy.toml` file is created in the current directory containing a unique project identifier, and the vault contains a matching project entry.
2. **Given** a directory that already has `envy.toml`, **When** the user runs `envy init`, **Then** the command fails with a clear message explaining the project is already initialised (no overwrite).
3. **Given** a subdirectory of an already-initialised project, **When** the user runs `envy init`, **Then** the command warns the user that a parent project already exists and stops without creating a nested manifest.

---

### User Story 2 - Secret CRUD via Terminal (Priority: P1)

A developer manages secrets for a project through the terminal. They can add or update secrets (`set`), read a specific secret's value (`get`), list all secret key names for an environment (`list`/`ls`), and delete secrets (`rm`/`remove`). All operations target a specific environment, defaulting to `development` when none is specified.

**Why this priority**: This is the core value proposition of Envy. Without CRUD, the tool cannot be used for its primary purpose.

**Independent Test**: Can be fully tested by running `set`, `get`, `list`, and `rm` against an initialised project, verifying values round-trip correctly and the list never exposes secret values.

**Acceptance Scenarios**:

1. **Given** an initialised project, **When** the user runs `envy set API_KEY=secret123`, **Then** the secret is stored encrypted in the `development` environment and the command exits with code 0.
2. **Given** a stored secret `API_KEY` in `development`, **When** the user runs `envy get API_KEY`, **Then** the exact decrypted value is printed to stdout and nothing else.
3. **Given** a stored secret `API_KEY` in `staging`, **When** the user runs `envy get API_KEY -e staging`, **Then** the staging value is returned, not the development value.
4. **Given** three stored secrets, **When** the user runs `envy list` or `envy ls`, **Then** only the key names are printed (alphabetically), never the values.
5. **Given** a stored secret, **When** the user runs `envy rm API_KEY`, **Then** the secret is deleted and subsequent `get` returns a not-found error.
6. **Given** a value containing `=` (e.g., `envy set TOKEN=abc=def`), **When** the command is parsed, **Then** only the first `=` is used as the separator, so the key is `TOKEN` and the value is `abc=def`.
7. **Given** `envy set =NOKEY`, **When** the command is executed, **Then** the command fails with a clear message about an invalid key name.

---

### User Story 3 - Process Injection (Priority: P1)

A developer runs their application with secrets injected directly into the process environment — without ever writing those secrets to a file. They use `envy run -- <command>` and the tool decrypts all secrets for the selected environment, passes them as environment variables to the child process, and proxies the child's exit code exactly.

**Why this priority**: This is the "magic wrapper" that makes Envy competitive with `.env` files. Without it, users have no way to actually consume secrets in their applications.

**Independent Test**: Can be fully tested by running `envy run -- printenv` and verifying that a previously set secret appears in the output, and that the process exits with the child's exit code.

**Acceptance Scenarios**:

1. **Given** secrets `A=1` and `B=2` stored in `development`, **When** the user runs `envy run -- env`, **Then** `A=1` and `B=2` appear in the printed environment and the command exits with code 0.
2. **Given** secrets in `staging`, **When** the user runs `envy run -e staging -- node server.js`, **Then** only staging secrets are injected, not development secrets.
3. **Given** a child command that exits with code 42, **When** the user runs `envy run -- ./script.sh`, **Then** `envy` exits with code 42.
4. **Given** a child command that fails to launch (binary not found), **When** the user runs `envy run -- nonexistent-bin`, **Then** `envy` exits with a non-zero code and prints a clear error message.
5. **Given** an environment with no secrets, **When** the user runs `envy run -- <command>`, **Then** the command runs successfully with no extra environment variables injected.

---

### User Story 4 - Legacy Migration (Priority: P2)

A developer with an existing `.env` file wants to migrate to Envy without manually re-entering every secret. They run `envy migrate .env` and the tool reads each `KEY=VALUE` pair and stores it in the vault. Comment lines and blank lines are skipped. Malformed lines produce a warning but do not abort the migration.

**Why this priority**: Migration reduces the adoption barrier significantly. Without it, users with existing projects must manually re-enter all secrets, which is error-prone and time-consuming. It is P2 because CRUD works standalone without it.

**Independent Test**: Can be fully tested by creating a `.env` file with 5 valid pairs, 2 comment lines, 1 blank line, and 1 malformed line, then running `envy migrate .env` and verifying all 5 valid pairs are readable via `get`.

**Acceptance Scenarios**:

1. **Given** a `.env` file with `KEY=VALUE` pairs, **When** the user runs `envy migrate .env`, **Then** each pair is stored in the `development` environment and the command reports how many secrets were imported.
2. **Given** a `.env` file with comment lines (`# comment`) and blank lines, **When** migration runs, **Then** those lines are silently skipped.
3. **Given** a `.env` file with a malformed line (no `=` separator), **When** migration runs, **Then** a warning is printed for that line but migration continues and all valid pairs are stored.
4. **Given** the `-e staging` flag, **When** the user runs `envy migrate .env -e staging`, **Then** all pairs are imported into the `staging` environment.
5. **Given** a `.env` file that does not exist, **When** the user runs `envy migrate missing.env`, **Then** the command fails immediately with a clear "file not found" message.

---

### Edge Cases

- What happens when `envy get` is run in a directory with no `envy.toml` in any ancestor? → Clear "not an envy project" error, no panic.
- What happens when the vault file is missing or corrupted? → Clear error message, no panic.
- What happens when the OS credential store is unavailable (no keychain)? → Clear error message explaining the vault key cannot be retrieved.
- What happens when `envy run` is called with no command after `--`? → Clear usage error, no panic.
- What happens when a secret key contains spaces or special characters? → `validate_key` rejects the key with a clear message before any vault access.
- What happens when `envy set` is called with the same key twice? → Second call overwrites the first (upsert semantics, last write wins).
- What happens when `envy list` is called on an environment that has never had any secrets? → Returns empty output (zero lines), exit code 0.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CLI MUST expose commands: `init`, `set`, `get`, `list` (alias `ls`), `rm` (alias `remove`), `run`, and `migrate`.
- **FR-002**: All commands except `init` MUST accept a global `-e, --env <ENV>` flag that selects the target environment; when omitted, the environment defaults to `development`.
- **FR-003**: The `set` command MUST accept arguments in `KEY=VALUE` format and split on the **first** `=` only, so values containing `=` are preserved correctly.
- **FR-004**: The `list` / `ls` command MUST print only key names, never decrypted values, sorted alphabetically.
- **FR-005**: The `run` command MUST inject all secrets for the selected environment as environment variables into the child process and proxy the child's exit code exactly as received.
- **FR-006**: The `migrate` command MUST parse a plaintext `.env` file line-by-line: store valid `KEY=VALUE` pairs, skip `#`-prefixed comments and blank lines, and emit a per-line warning for malformed entries without aborting.
- **FR-007**: The CLI layer MUST be the sole owner of the Vault connection lifecycle and the master key retrieval; it MUST pass only references (`&Vault`, `&[u8; 32]`) to Core functions — never the key itself as a heap string.
- **FR-008**: All user-visible errors MUST be formatted as clean, human-readable terminal messages; raw `Debug` output or stack traces MUST NOT be shown to the user.
- **FR-009**: The `init` command MUST fail with a clear message if `envy.toml` already exists in the current directory.
- **FR-010**: The `init` command MUST warn and stop if a parent directory already contains an `envy.toml` manifest.
- **FR-011**: The `get` command MUST print **only** the secret value to stdout (no labels, no trailing metadata), so its output can be used in shell pipelines.
- **FR-012**: The `run` command MUST accept an arbitrary command and its arguments after the `--` separator; the `--` itself is consumed by the CLI parser and not passed to the child process.

### Key Entities

- **Command**: One of the 7 named subcommands exposed by the CLI. Each command maps to one or more Core functions and produces human-readable output on stdout/stderr.
- **Environment**: A named context (e.g., `development`, `staging`, `production`) selected at runtime via `-e / --env`. Defaults to `development` when not specified.
- **Secret**: A `KEY=VALUE` pair stored encrypted in the vault and associated with a project and environment. Keys must be non-empty and must not contain `=`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 7 commands complete their primary task in under 200 ms on a developer laptop (excluding child-process runtime for `run`).
- **SC-002**: The `run` command adds less than 50 ms of overhead compared to running the child process directly, measured as wall-clock time from `envy run` invocation to first byte of child output.
- **SC-003**: `envy migrate` imports a 50-line `.env` file in under 1 second end-to-end.
- **SC-004**: Zero plaintext secret values appear in any output of `list` or on stderr during normal operation.
- **SC-005**: Every error scenario (missing manifest, bad key, vault unavailable) produces a human-readable message and exits with a non-zero code — no panics in any code path.
- **SC-006**: The `run` command's exit code matches the child process exit code in 100% of tested scenarios, including non-zero exit codes and signal termination.

## Known Bugs & Fixes

### BUG-001: `envy run` crashes on a fresh project with no secrets (fixed)

**Reported**: 2026-03-20
**Status**: Fixed in `src/core/ops.rs`

**Symptom**: Running `envy run -- <cmd>` immediately after `envy init`, before any `envy set` call, exited with `error: record not found` instead of executing the command.

**Root cause**: `get_env_secrets()` called `vault.get_environment_by_name()` and propagated `DbError::NotFound` via `?`. On a fresh project no environment row exists yet (environments are auto-created lazily by `set_secret`, not by `init`), so the lookup always fails before any secrets have been stored.

**Fix**: Handle `DbError::NotFound` explicitly in `get_env_secrets()` — return `Ok(HashMap::new())` instead of propagating the error. A missing environment is semantically equivalent to an environment with zero secrets for read-only operations.

**Regression test added**: `core::ops::tests::get_env_secrets_missing_env_returns_empty`

---

## Assumptions

- The master key is retrievable from the OS credential store before any command that needs vault access. Commands that cannot retrieve the key fail fast with a clear message.
- The vault file (`~/.envy/vault.db`) already exists or is created automatically on first use (managed by the DB layer, not the CLI).
- Environment names are case-insensitive at input but normalised to lowercase in storage (handled by Core).
- The `run` command does not inherit `envy`'s own process environment into the child beyond what the OS already provides; it only *adds* the decrypted secrets on top of the inherited environment.
- Signal forwarding (SIGINT, SIGTERM) to the child process is out of scope for this feature; basic exit-code proxying is sufficient for the MVP.
