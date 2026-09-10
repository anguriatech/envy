# Feature Specification: Transparent Shell Auto-Injection

> **SUPERSEDED by `specs/018-transparent-shims/spec.md` (ADR-001).**
> The prompt-hook export mechanism described below was implemented on an
> unmerged branch and replaced before merge: exporting secrets into the
> parent shell violates envy's containment guarantee. The motive, the
> `auto_inject` manifest flag, `envy auto`, `envy shell-init`, and the manifest
> tests were kept; hook plan/render/escaping and the rc installer were
> dropped in favour of PATH shims. This document is retained for traceability.

**Feature Branch**: `017-shell-auto-inject`
**Created**: 2026-09-09
**Status**: Superseded
**Input**: User description: "Today envy only injects secrets when commands are prefixed with `envy run --`. People (and AI agents) keep using the legacy `.env` flow because neither knows the prefix is required. After `envy init`, entering the project should inject envy variables instead of `.env` ones, with no prefix on every command."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Opt a project in or out (Priority: P1)

A developer runs `envy init --auto-inject` (or `envy auto on` in an existing
project) and the project's `envy.toml` carries `auto_inject = true`. `envy auto`
with no argument reports the state; `envy auto off` disables it again.
Manifests written before this feature parse as opted-out.

**Why this priority**: The opt-in flag is the safety core — without it the hook
cannot distinguish "inject here" from "leave the shell alone".

**Independent Test**: `create_manifest_with_options` / `set_manifest_auto_inject`
round-trip plus `auto on|off|status` output assertions (unit + ignored E2E).

**Acceptance Scenarios**:

1. **Given** a fresh directory, **When** the user runs `envy init --auto-inject`,
   **Then** `envy.toml` contains `auto_inject = true` and the command exits 0.
2. **Given** a project initialised with plain `envy init`, **When** the user runs
   `envy auto status`, **Then** it reports `off`; after `envy auto on` it reports
   `on` and the flag survives re-reading the file (comments and
   `rotation_reminder_days` preserved).
3. **Given** an `envy.toml` written by an older envy (no `auto_inject` line),
   **When** any command reads the manifest, **Then** it parses with auto-inject
   off (backward compatible, no migration).

---

### User Story 2 - One-time shell setup, offered on the spot (Priority: P1)

After opting in, the developer needs the shell to call envy on every directory
change. `envy shell-init <shell>` prints the snippet for bash/zsh/fish/
powershell/nushell (direnv users put `eval "$(envy hook --shell bash)"` in
`.envrc`). `envy init --auto-inject` and `envy auto on` detect `$SHELL` and ask
`[y/N]` (default `N`) whether to append the hook to the rc file.

**Why this priority**: Without the hook installed, the flag alone does nothing —
this is the step users would otherwise skip.

**Independent Test**: `append_snippet_if_missing` idempotency tests;
`hook_already_installed` detection tests; manual E2E on macOS zsh.

**Acceptance Scenarios**:

1. **Given** `auto_inject = true` was just set, **When** the installer runs on an
   interactive terminal with `$SHELL` = zsh, **Then** it asks to append to
   `~/.zshrc`; answering `y` appends the full snippet once, answering `n`
   prints the manual one-liner instead.
2. **Given** the hook is already installed (snippet marker or hand-added hook
   line for that shell), **When** the installer runs, **Then** it reports
   "already present" and never duplicates — whatever the answer.
3. **Given** no TTY (CI pipe), **When** `envy init --auto-inject` runs,
   **Then** it exits 0 without prompting and prints the manual one-liner.
4. **Given** powershell/nushell (no deterministic rc path), **When** the
   installer runs, **Then** it prints the manual step instead of prompting.

---

### User Story 3 - Secrets follow the directory, prefix-free (Priority: P1)

With the hook installed, entering the project directory exports the vault
secrets into the shell (`npm run dev`, `npx expo run:ios`, `python …` all see
them — every child inherits the parent environment). Leaving unloads them via
the `$__ENVY_KEYS` tracking variable. Environment selection is `--env` flag >
`$ENVY_ENV` > `development`; `ENVY_AUTO_INJECT=0` forces unload-only mode.

**Why this priority**: This is the feature — transparent injection for humans
and agents that never learned the `envy run --` prefix.

**Independent Test**: `compute_hook_plan` / `render_hook_plan` unit tests per
shell; ignored E2E asserting `export FOO=` in hook output after `set`.

**Acceptance Scenarios**:

1. **Given** auto-inject on with secret `FOO` set, **When** the shell enters the
   project, **Then** `FOO` is exported (visible to any child process) without
   any prefix.
2. **Given** the shell leaves the project tree, **When** the hook runs,
   **Then** previously exported keys are unset and tracking vars cleared.
3. **Given** `ENVY_ENV=staging`, **When** the hook runs, **Then** staging
   secrets are injected (flag `--env` wins over the variable).
4. **Given** `ENVY_AUTO_INJECT=0`, **When** the hook runs inside the project,
   **Then** nothing is injected (unload only).
5. **Given** no manifest, flag off, or an unreachable vault/keyring,
   **When** the hook runs, **Then** it exits 0 with unload-cleanup or empty
   output — never an error, so the prompt survives.

---

### User Story 4 - Envy wins over legacy `.env`, warned once (Priority: P2)

Exported variables live in the parent environment, which every major dotenv
loader refuses to overwrite — envy wins by construction. When a `.env` sits
next to the manifest, one stderr warning is printed per transition (entering,
env switch, vault change), never twice per `cd` nor on every prompt.

**Why this priority**: The warning is the migration signal; spamming it on
every prompt would train users to ignore warnings entirely.

**Independent Test**: `hook_state_changed` unit tests; manual E2E (single
warning on entry, silence on Enter, warning again on re-entry).

**Acceptance Scenarios**:

1. **Given** `.env` with `FOO=old` next to the manifest and vault `FOO=new`,
   **When** the shell enters, **Then** exactly one warning prints and `FOO`
   evaluates to `new`.
2. **Given** steady state inside the project, **When** the user presses Enter,
   **Then** no warning repeats.

---

### User Story 5 - Documentation (Priority: P3)

Per-command pages `envy-auto`, `envy-shell-init`, `envy-hook` under
`docs/commands/`; `envy-init`/`envy-run` cross-link them; README quickstart and
command table mention the flow.

**Why this priority**: No functional risk; required so humans and agents can
discover the flow without reading source.

**Independent Test**: Reviewer follows README + new pages to complete setup.

**Acceptance Scenarios**:

1. **Given** the README/docs, **When** a reader follows them, **Then** they can
   complete opt-in → shell setup → prefix-free run without reading code.

---

### Edge Cases

- **Vault key is not a valid shell identifier** (`BAD-KEY`): skipped with a
  stderr warning listing the key; never interpolated (also applies to a
  poisoned `$__ENVY_KEYS`, which is validated before unsetting).
- **Empty environment / missing project row** (fresh machine): degrades to
  unload (empty map), not an error.
- **Existing rc content without trailing newline**: newline fixed before
  appending; original bytes otherwise preserved.
- **Re-running the installer**: marker- and shell-aware detection — a bash
  snippet does not count as a zsh installation, but never stacks duplicates.
- **GUI-launched apps / `sudo`**: out of scope — they don't inherit the
  interactive shell environment (`envy run` remains the answer there, as in CI).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `envy.toml` MUST carry `auto_inject: bool`, defaulting to `false`
  for manifests that omit it (backward compatible, no migration).
- **FR-002**: `create_manifest` MUST keep its signature (writes `false`);
  `create_manifest_with_options` MUST accept the flag; `set_manifest_auto_inject`
  MUST edit textually (replace existing line, else insert after `project_id`),
  preserving unrelated settings and comments.
- **FR-003**: `envy init` MUST accept `--auto-inject`; `envy auto [on|off|status]`
  MUST manage the flag (`status` when omitted).
- **FR-004**: `envy shell-init [bash|zsh|fish|powershell|nushell]` MUST print a
  static snippet (no vault/manifest access); default shell detected from
  `$SHELL`, else bash.
- **FR-005**: `envy hook [--shell SHELL] [-e ENV]` MUST print `eval`-able code
  to stdout and warnings to stderr, and MUST always exit 0.
- **FR-006**: Key names from the vault AND from `$__ENVY_KEYS` MUST be validated
  (`^[A-Za-z_][A-Za-z0-9_]*$`) before interpolation; values MUST be
  single-quote escaped per shell (POSIX / fish / PowerShell); nushell MUST emit
  JSON applied via `load-env`.
- **FR-007**: The installer offer MUST default to `N`, MUST NOT prompt without
  a TTY, MUST be idempotent, and MUST degrade to printed instructions on any
  failure — `init`/`auto on` MUST never fail retroactively because of it.
- **FR-008**: Entry warnings MUST fire once per injected-set transition
  (`hook_state_changed`), never unconditionally per invocation.
- **FR-009**: All new unit tests MUST run without an OS keyring; keyring paths
  MUST be covered by `#[ignore]`d E2E tests following `cli_integration.rs`
  convention.

### Key Entities

- **Manifest flag**: `auto_inject` in `envy.toml` — per-project opt-in.
- **HookPlan**: pure export/unload computation (secrets, unset, keys, skipped).
- **Tracking variables**: `$__ENVY_KEYS` (unload list), `$__ENVY_ENV` (info).
- **Installer**: shell detection, rc paths, marker-based idempotency.

## Success Criteria *(mandatory)*

- **SC-001**: `cargo test` green (incl. 17 shell unit tests + manifest
  round-trip), `cargo clippy -- -D warnings` clean, `cargo audit` clean.
- **SC-002**: Manual E2E on macOS zsh: opt-in → install (`y`) → restart →
  secret visible prefix-free → unload on leave → single `.env` warning on entry.
- **SC-003**: 100% of pre-existing subcommand outputs byte-identical (no output
  contract changed except additive `init` hint lines).
- **SC-004**: Prompt hook can never break the shell: every failure mode degrades
  to unload-cleanup or silence with exit 0 (unit + E2E covered).
- **SC-005**: No secret value or invalid key name reaches `eval` uninterpolated-
  safe paths (validation + per-shell escaping, unit-tested incl. poisoning).

## Assumptions & Confirmed Decisions

- Transparent interception of arbitrary child spawns is impossible on Unix;
  parent-shell cooperation (hook à la direnv/mise) is the standard mechanism.
- Opt-in per project (`auto_inject`), one-time setup per machine — `init` never
  modifies the shell without an explicit `y`.
- Auto-install only for bash/zsh/fish (deterministic rc paths); powershell/
  nushell stay manual.
- `envy run` remains the scoped choice for CI/production (documented tradeoff,
  not a deprecation).
- No new crates, no new exit codes, no new `CliError` variants.

## Non-Goals

- No `AGENTS.md`/`CLAUDE.md` generation for user projects (explicitly deferred).
- No GUI/IDE-run-button integration (they don't inherit the interactive shell).
- No background daemon or file watching — hook runs on `chpwd`/`precmd` only.
- No `envy auto` installer offer divergence from `init` (same function).
