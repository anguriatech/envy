# Feature Specification: Documentation, Examples & Demos

**Feature Branch**: `015-docs-examples-demos`  
**Created**: 2026-08-03  
**Status**: Draft  
**Input**: User description: "Documentation & adoption workstream — README as entry point, per-command docs pages, CI-verified examples, VHS demo videos."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Per-command documentation pages (Priority: P1)

A developer lands on the envy README, sees the command reference table, and clicks any command name to open a dedicated page that explains what the command does, its syntax and flags, typical usage examples, how it works under the hood (including security implications), and its specific exit codes.

**Why this priority**: Documentation is the #1 adoption lever with zero code risk. Without per-command pages, users must reverse-engineer behavior from a dense single README and tend to abandon on first friction. This is the core deliverable.

**Independent Test**: Every one of the 17 commands has a page under `docs/commands/`, the README command table links to each page, and every link resolves to an existing file.

**Acceptance Scenarios**:

1. **Given** a user reading the README "Full Command Reference", **When** they click any command name, **Then** they are taken to a page under `docs/commands/` covering: what it does, syntax + flags table, typical examples (code blocks), how it works (security/memory/exit codes), and related commands.
2. **Given** the README, **When** a link checker validates the command table links, **Then** all 17 links resolve to existing `.md` files.

---

### User Story 2 - CI-verified copy-paste examples (Priority: P1)

A user wants to try envy in a realistic setup without reading the full command reference. They open `examples/` and find four ready-to-run scenarios (basic, team-sync, ci-cd, monorepo), each with a README and a runnable script. They run the script against their own `envy` binary and it works. The examples never rot: CI executes them on every push.

**Why this priority**: Examples that break in CI erode trust. Making them CI-verified guarantees they stay accurate and gives users copy-paste confidence, which directly drives onboarding success.

**Independent Test**: A new E2E scenario (Scenario 15) runs each example's script with a real `envy` binary and asserts expected outputs; all scenarios must pass in CI.

**Acceptance Scenarios**:

1. **Given** an `examples/` directory with `basic/`, `team-sync/`, `ci-cd/`, `monorepo/` subdirectories, **When** CI runs the E2E script, **Then** each example executes successfully against the built binary and produces the documented outputs.
2. **Given** a user following an example's README, **When** they run the example's script, **Then** it is idempotent and completes without manual secrets.

---

### User Story 3 - Hero-flow demo videos (Priority: P2)

A new user can watch a short recording of the three most valuable flows: quickstart (init → set → run), team sync (encrypt → decrypt across two developers), and CI/CD headless usage. Videos are generated from versioned VHS tapes in CI and committed back to the repo, so they never drift from the actual CLI behavior.

**Why this priority**: Video demos lower the perceived complexity of a security tool dramatically. VHS tapes are deterministic and CI-generated, keeping them fresh without manual recording. Lower priority than docs/examples because they are a polish layer over the same flows.

**Independent Test**: A CI job (vhs-action) runs the three tapes and auto-commits the generated GIFs; the tapes live in `vhs/` and reference the real binary.

**Acceptance Scenarios**:

1. **Given** the `vhs/` directory containing `quickstart.tape`, `team-sync.tape`, and `ci-cd.tape`, **When** CI executes the VHS job, **Then** GIFs are generated into `docs/assets/` and committed (or the job fails loudly).
2. **Given** a user on the README, **When** they open the demos section, **Then** they can watch the three hero flows without installing anything.

---

### User Story 4 - Updated developer guide (Priority: P3)

A contributor opening `docs/developer-guide.md` finds the architecture overview matches the current codebase, including the `scan`, `audit`, and `sync_markers` modules added in recent features.

**Why this priority**: Stale contributor docs increase onboarding friction for new maintainers and erode confidence. Lowest priority because it has no user-facing impact.

**Independent Test**: The guide's module listing includes every module present under `src/` and `src/db/`.

**Acceptance Scenarios**:

1. **Given** the current source tree, **When** a reader scans the guide's architecture section, **Then** all existing modules (including scan/audit/sync_markers) are listed.

---

### Edge Cases

- **Command pages for commands with no visible output** (`set`, `rm`, `migrate`): pages must still document syntax, flags, exit codes, and security notes; a "no output on success" note is expected.
- **Alias coverage**: pages must document aliases (`ls`, `remove`, `unset`, `enc`, `dec`, `df`, `st`, `au`) so users searching by alias find the canonical page.
- **Monorepo example on non-git directories**: `envy run`/`hooks` behaviors differ without a git repo — example README must state prerequisites.
- **VHS CI failures**: a failing video generation must fail the CI job, not silently skip, so tapes can never drift silently.
- **Auto-commit loop**: the VHS job must not re-commit unchanged GIFs (no-op if no diff) to avoid infinite CI loops.
- **Examples on 3 OS**: CI matrix runs Scenario 15 on macOS/Linux/Windows; scripts must be POSIX-compatible or explicitly gated.
- **Existing E2E scenarios must not regress**: Scenario 15 is additive; the current 14 scenarios / 114 assertions keep passing.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: README "Full Command Reference" MUST link each of the 17 commands (`init`, `set`, `get`, `list`, `rm`, `run`, `migrate`, `encrypt`, `decrypt`, `export`, `diff`, `status`, `rotate`, `scan`, `audit`, `hooks`, `completions`) to a dedicated page under `docs/commands/`.
- **FR-002**: Each command page MUST follow a shared template (`docs/commands/_template.md`) with sections: What it does; Aliases; Syntax & flags; Typical examples (code blocks); How it works (security, memory, exit codes); Related commands.
- **FR-003**: Command pages MUST use code blocks only in this iteration — no embedded media of any kind. Static screenshots for output-heavy commands (`status`, `diff`, `scan`, `audit`, `export`) are deferred to a follow-up PR (see Assumptions).
- **FR-004**: The repo MUST include `examples/` with four scenarios: `basic/`, `team-sync/`, `ci-cd/`, `monorepo/`, each with a README and an idempotent bash script runnable with `ENVY_BIN`.
- **FR-005**: `tests/e2e_devops_scenarios.sh` MUST gain a "Scenario 15 — Documentation examples" that runs each example script and asserts expected outputs using the existing `assert_eq` harness.
- **FR-006**: The repo MUST include `vhs/` with exactly three tapes: `quickstart.tape`, `team-sync.tape`, `ci-cd.tape`, recorded headless via `ENVY_PASSPHRASE` with the real binary.
- **FR-007**: CI MUST run VHS generation (charmbracelet/vhs-action) on push to `master` (not on PRs) and auto-commit generated GIFs to `docs/assets/`, failing the job on tape errors and skipping the commit when nothing changed.
- **FR-008**: `docs/developer-guide.md` MUST be updated so its architecture overview lists all current modules including `scan`, `audit`, `sync_markers`.
- **FR-009**: No changes to CLI behavior, command contracts, exit codes, or Rust source are allowed; only docs, example scripts, tests, tapes, and CI workflows may change.
- **FR-010**: A CI check MUST verify (a) every command-table link in the README resolves to an existing file under `docs/commands/`, and (b) every page contains the required template sections (grep-based check, e.g. in the E2E script or a lightweight workflow step).
- **FR-011**: The README MUST include new sections linking to: the `docs/commands/` index, the `examples/` directory, and the hero-flow demo videos.

### Key Entities *(include if feature involves data)*

- **Command page**: A markdown document under `docs/commands/` describing one CLI command (what/how/syntax/examples/exit codes/related).
- **Example scenario**: A directory under `examples/` with README + runnable script, CI-verified.
- **VHS tape**: A declarative recording script under `vhs/` that deterministically produces a demo GIF.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 17/17 command pages exist and 100% of README command-table links resolve to existing files.
- **SC-002**: 100% of command pages conform to the shared template (all required sections present, verified by review or script).
- **SC-003**: All 4 example scenarios pass in CI on the 3-OS matrix (E2E Scenario 15 green) without regressing the existing 14 scenarios / 114 assertions.
- **SC-004**: 3/3 hero-flow GIFs are generated by CI from versioned tapes, and the VHS job passes without manual intervention.
- **SC-005**: A new user can complete the quickstart (init → set → run) using only the README within 5 minutes (verified by the CI execution of the `basic` example).
- **SC-006**: The `envy --help`/command behavior is byte-identical before and after this feature (zero CLI changes).

## Assumptions & Confirmed Decisions

- **Plain Markdown, no static site**: command pages live under `docs/commands/` and are linked from the README; no mdBook/VitePress.
- **VHS in CI**: tapes are the source of truth; generation runs via vhs-action **on push to master only** (not on PRs — fork PRs cannot push) with auto-commit of GIFs (no-op commit when unchanged).
- **Three hero tapes only**: quickstart, team-sync, ci-cd. Other commands get code blocks; static screenshots for `status`/`diff`/`scan`/`audit`/`export` are deferred to a follow-up PR.
- **Lightweight spec process**: only `spec.md`, `plan.md`, `tasks.md` are produced for this feature (no research.md, data-model.md, contracts/, quickstart.md).
- **Headless demo strategy**: tapes use `ENVY_PASSPHRASE` and the real binary to avoid keyring prompts (deterministic, ephemeral vault).
- **No CLI changes**: this feature is strictly additive to docs/tests/examples/CI.
