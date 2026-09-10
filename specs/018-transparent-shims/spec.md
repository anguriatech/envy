# Feature Specification: Transparent Command Shims

**Feature Branch**: `018-transparent-shims`
**Created**: 2026-09-10
**Status**: Draft
**Supersedes**: `017-shell-auto-inject` (prompt-hook export design — implemented
on an unmerged branch, never committed as a spec; see ADR below for why the
mechanism was replaced while the motive and most of the code are kept)
**Input**: User description: "Remove the mandatory `envy run --` prefix without
breaking envy's containment (parent shell must never hold secrets). Reviewer
direction: rbenv/asdf/mise-style shims — `~/.envy/shims/` first on `PATH`,
each shim delegating to hidden `envy exec` plumbing with `run`-grade scoping."

## ADR-001 — hook-vs-shim (decision record, mandatory reading)

**Context.** Spec 017 implemented transparent injection via a shell hook that
exported vault secrets into the interactive parent shell (`eval "$(envy hook)"`
on `chpwd`/`precmd`). It worked for every command with zero per-command setup.

**Decision.** Replace the mechanism with PATH shims; drop the parent-export.

**Why.** On Unix a process environment comes from its parent at spawn or from
itself — there is no fourth place. Transparent + total + parent-clean is
structurally impossible; at most two hold:
- total + transparent = hook (breaks containment),
- containment + transparent = shims (partial coverage),
- total + containment = `envy run --` (keeps the prefix).
Containment (secrets only in the child subtree, zeroized, never in the shell)
is envy's differentiator — direnv-with-a-vault is not the product. Shims are
additionally better for scripts and non-interactive agents (they inherit `PATH`
but never run prompt hooks) and kill the 5-shell escaping matrix.

**Accepted costs (honest).** (1) Coverage is per-command: the long tail
(`./deploy.sh`, `docker`, project binaries) still needs `envy run` or
`shim add` — mitigated by generous detectors + `doctor`. (2) Silent PATH-order
bypass: version managers that prepend (fnm/nvm/mise/volta) can shadow shims —
mitigated by installer ordering guidance + mandatory `doctor` checks. (3) Cost
moves from per-prompt to per-command vault open. (4) `docker` is excluded from
auto-detection on purpose: injecting into `docker build` can leak secrets into
image layers (manual `shim add docker` stays possible).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One-time machine setup (Priority: P1)

A developer runs `envy shell-init zsh`, pastes the printed
`export PATH="$HOME/.envy/shims:$PATH"` line LAST in `~/.zshrc`, restarts the
shell. `which -a npm` lists the shim first; `envy doctor` is green. The line is
never installed automatically.

**Independent Test**: `shell-init` output assertions per shell (no keyring);
manual E2E on macOS/Linux/Windows.

**Acceptance Scenarios**:

1. **Given** any shell, **When** the user runs `envy shell-init <shell>`,
   **Then** it prints a static `PATH`-prepend line (shell syntax) with an
   ordering warning, and exits 0 without touching vault, manifest, or rc files.
2. **Given** the line installed last, **When** the user runs `which -a npm`,
   **Then** `~/.envy/shims/npm` resolves first.

---

### User Story 2 - Per-project opt-in and shim generation (Priority: P1)

The developer runs `envy init`, `envy auto on` (reused `auto_inject` flag from
017), `envy reshim`. Detectors map project files to commands (`package.json` →
npm/npx/node, `Cargo.toml` → cargo, …) and create dumb shims delegating to
`envy exec`. Outside opted-in projects (or with `auto off`) shims pass through
transparently with zero vault access.

**Independent Test**: detector unit tests; `reshim` hermetic E2E (redirected
HOME, hand-written manifest — no keyring); manual E2E per OS.

**Acceptance Scenarios**:

1. **Given** a project with `package.json` and `auto on`, **When** the user
   runs `envy reshim`, **Then** `~/.envy/shims/{npm,npx,node}` (+`.cmd` twins on
   Windows) exist, are executable, and re-running changes nothing.
2. **Given** no manifest, **When** the user runs `envy reshim`, **Then** it
   exits 1 (`ManifestNotFound`) without touching the vault or keyring.
3. **Given** an opted-out project, **When** a shimmed command runs, **Then** it
   behaves exactly as the real binary (no vault open, no keyring touch).
4. **Given** `envy reshim --prune` after removing `package.json`, **When** it
   runs, **Then** auto-provenance shims for npm/npx/node are removed while
   manually added ones survive.

---

### User Story 3 - Prefix-free daily use with a clean parent (Priority: P1)

Inside the project, `npm run dev` injects vault secrets scoped to the child
subtree with `run`-grade guarantees (exact exit code, zeroized memory);
`env | grep API_KEY` in the parent is always empty. Vault/keyring failures are
loud (stderr, non-zero) — never silent.

**Independent Test**: `exec` resolution unit tests; ignored E2E through a real
shim on `PATH`; manual E2E incl. parent-clean assertion.

**Acceptance Scenarios**:

1. **Given** auto on with secret `FOO`, **When** the user runs shimmed `npm …`,
   **Then** the child sees `FOO`, the parent does not, and the exit code is
   proxied exactly.
2. **Given** an unreachable vault/keyring, **When** a shimmed command runs in
   an opted-in project, **Then** it prints a visible stderr error and exits 4
   (never runs naked silently).
3. **Given** a shimmed command outside any envy project, **When** it runs,
   **Then** it execs the real binary directly (zero overhead, no keyring).

---

### User Story 4 - Manual shims and diagnostics (Priority: P2)

`envy shim add expo` covers the long tail; `rm`/`list` manage it.
`envy doctor` reports setup health (exit 0 clean / 1 findings, never touches
the vault): shims on `PATH`, order vs version managers, project coverage
(`reshim` hint), per-command resolution for given names.

**Independent Test**: hermetic `shim add/rm/list` E2E (redirected HOME, unique
names + cleanup); `doctor` E2E with controlled `PATH`; unit tests for
validators and resolution.

**Acceptance Scenarios**:

1. **Given** any directory, **When** the user runs `envy shim add mytool`,
   **Then** a manual-provenance shim is created (idempotent message if present);
   invalid names exit 2 without writing.
2. **Given** shims missing from `PATH`, **When** `envy doctor` runs, **Then** it
   exits 1 naming the fix.
3. **Given** a manager dir (e.g. fnm) before shims on `PATH`, **When** `doctor`
   runs, **Then** it exits 1 warning about shadowing.
4. **Given** `envy doctor npm` where npm resolves outside shims, **When** it
   runs, **Then** it warns the command runs WITHOUT secrets and suggests
   `shim add` or `envy run`.

---

### User Story 5 - Documentation (Priority: P3)

`docs/commands/` gains `envy-reshim`, `envy-shim`, `envy-exec`, `envy-doctor`;
`envy-shell-init` is rewritten (PATH lines), `envy-auto`/`envy-init` point at
`reshim`; the hook page is removed; README quickstart + table follow.

**Independent Test**: Reviewer completes setup from docs alone.

### Edge Cases

- **Recursion**: shims resolve the real binary with the shims dir stripped from
  `PATH`; `envy run -- npm …` through a shim double-injects harmlessly.
- **Explicit paths** (`./x`, `/bin/y`) bypass shim logic by definition (used
  as-is).
- **Empty `envy exec` invocation**: exit 127, no panic (clap guarantees, plus a
  defensive guard per the no-panics policy).
- **Unwritable shims dir**: loud error, non-zero exit (setup-time, never silent).
- **Windows**: `.cmd` twins (CRLF); no extensionless files (a bare file would
  shadow `.cmd` under `CreateProcess` semantics); PowerShell runs `.cmd`
  without ExecutionPolicy friction.
- **`sudo`**: out of scope — root does not inherit user shims (injecting to
  root by default would be worse).
- **npm-in-npm** (lifecycle spawning `npx`/`sh`): re-resolves through shims,
  re-injects identically — correct, small overhead.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `auto_inject` manifest flag keeps 017 semantics, repointed:
  opt-in for shim injection (`create_manifest_with_options`,
  `set_manifest_auto_inject` unchanged).
- **FR-002**: `~/.envy/shims/` holds one dumb shim per command: POSIX shell
  script (executable bit, LF) on Unix, `.cmd` twin (CRLF) on Windows; both
  delegate to hidden `envy exec -- <name> "$@"` (`%*` on Windows) with a
  parseable provenance header (`envy-shim-source: auto|manual`).
- **FR-003**: `envy reshim [--prune]` MUST work without vault/keyring access
  (manifest read + fs only); exit 1 without manifest; `--prune` removes only
  auto-provenance shims not detected in the current project.
- **FR-004**: Detector table v1: `package.json`→npm/npx/node,
  `yarn.lock`→yarn, `pnpm-lock.yaml`→pnpm, `Cargo.toml`→cargo,
  `pyproject.toml`→python/uv, `requirements.txt|setup.py`→python/pip,
  `go.mod`→go, `Makefile|makefile|GNUmakefile`→make, `justfile|.justfile`→just,
  `Gemfile`→bundle, `composer.json`→composer. `docker` excluded by default
  (image-layer leak risk; manual add allowed).
- **FR-005**: `envy exec` (hidden) MUST resolve outside the shims dir
  (anti-recursion; explicit paths pass through), inject scoped exactly like
  `run` when opted in (audited as `run`), pass through directly otherwise
  (zero vault access), fail loud (stderr, 127 not-found / 4 vault).
- **FR-006**: `envy shim add|rm|list` MUST be global (no manifest needed);
  names validated (`[A-Za-z0-9_.-]+`, no leading dot, not `.`/`..`) → exit 2
  (`InvalidShimName`, no new exit code); `rm` of a missing shim exits 1.
- **FR-007**: `envy doctor [CMD...]` MUST never touch the vault; checks PATH
  presence, order vs known prependers (fnm/nvm/volta/mise/rbenv/pyenv/asdf),
  project coverage (auto on + missing shims → `reshim` hint), per-command
  resolution; exit 0 clean / 1 findings.
- **FR-008**: `envy shell-init` MUST print static PATH lines only (no vault,
  manifest, rc, or prompt I/O); never auto-install (user pastes, keeps last).
- **FR-009**: Shim names, PATH parsing, and resolution MUST be unit-tested
  without keyring; vault paths covered by `#[ignore]`d E2E + hermetic
  non-ignored E2E (redirected HOME, unique names, cleanup) where no vault is
  involved.

### Key Entities

- **Shim**: dumb delegating executable in `~/.envy/shims/` (POSIX/`.cmd`).
- **Detector**: trigger files → commands table (project root = manifest dir).
- **Provenance**: `auto` vs `manual` header marker (prune safety).
- **Doctor finding**: PATH/order/coverage diagnostic (exit 1, never silent).

## Success Criteria *(mandatory)*

- **SC-001**: `cargo test` green, `cargo clippy -- -D warnings` clean,
  `cargo audit` clean, `cargo fmt --check` clean (run locally — no toolchain
  in this environment).
- **SC-002**: Manual E2E per OS: PATH line last → `which -a` shows shim first →
  `doctor` green → prefix-free run with parent provably clean
  (`env | grep KEY` empty) → vault failure loud.
- **SC-003**: Pre-existing subcommand outputs byte-identical (only additive
  `init` hint-line changes, as in 017).
- **SC-004**: `exec` proxies exit codes exactly (incl. signal→1, missing→127),
  matching `run` semantics.
- **SC-005**: No new crates, no new exit codes (one new `CliError` variant
  mapping to existing code 2), no `CliError` surface removed.

## Assumptions & Confirmed Decisions

- Shims dir first on `PATH`, user-installed line kept LAST in rc (documented;
  enforced by nothing but `doctor`).
- `auto on`/`init --auto-inject` do NOT auto-run `reshim` (explicit steps per
  reviewer flow; `doctor` catches forgotten runs).
- `exec` reuses `cmd_run` for the inject path (single spawn/audit/exit-code
  implementation).
- Hook code (plan/render/escaping/rc-append) is deleted, not demoted — reviewer
  explicit; no export-to-parent mode ships.
- TUI untouched (no shim surface in v1).

## Non-Goals

- No shell-hook export mode (superseded design, see ADR-001).
- No `docker build`-time injection semantics (excluded detector).
- No daemon, no file watching, no `sudo` integration.
- No landing-page (`site/`) changes in v1.
