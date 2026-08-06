# Implementation Plan: Documentation, Examples & Demos

**Branch**: `015-docs-examples-demos` | **Date**: 2026-08-03 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/015-docs-examples-demos/spec.md`

## Summary

Turn the README into the entry document for envy and make adoption frictionless: link the "Full Command Reference" table to 17 dedicated pages under `docs/commands/` (shared template, code blocks only), ship four CI-verified copy-paste scenarios under `examples/` (basic, team-sync, ci-cd, monorepo) validated by a new E2E "Scenario 15", add three VHS hero-flow tapes (`vhs/`) whose GIFs are generated in CI on push to master and auto-committed to `docs/assets/`, add a CI check verifying README links and template sections (FR-010), and refresh the outdated `docs/developer-guide.md` architecture section. Zero changes to Rust source, CLI behavior, contracts, or exit codes (FR-009).

## Technical Context

**Language/Version**: N/A — no Rust source changes (FR-009); only Markdown, Bash, VHS tapes, and GitHub Actions YAML
**Primary Dependencies**: `charmbracelet/vhs-action` (CI demo generation); no new Cargo dependencies
**Storage**: N/A — no vault/artifact/schema involvement
**Testing**: `tests/e2e_devops_scenarios.sh` (existing harness: 14 scenarios, 114 assertions, helpers `section`/`assert_eq`/`assert_ne`/`assert_contains`/`assert_not_contains`/`assert_file_exists`/`init_project`, `ENVY_BIN` env var); Scenario 15 must follow this style
**Target Platform**: Linux, macOS, Windows (3-OS CI matrix; scripts must be POSIX-compatible / Git Bash-safe)
**Project Type**: CLI tool — documentation workstream
**Performance Goals**: N/A (docs); VHS GIFs should stay reasonably small (< ~2 MB each) to avoid repo bloat
**Constraints**: Code blocks only in command pages this iteration (FR-003); no embedded secrets in examples (use dummy values); VHS job runs on push to master only and must skip commit when unchanged (FR-007); all docs in English (Constitution V)
**Scale/Scope**: 17 command pages, 4 example scenarios, 3 VHS tapes, 1 new E2E scenario, 1 CI docs-check, 1 CI VHS job, 1 developer-guide refresh

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | PASS | No secrets in docs/examples (dummy values only); tapes run headless via `ENVY_PASSPHRASE` (ephemeral deterministic vault, no keyring prompts); no Rust code touched |
| II. Determinism | PASS | VHS tapes are declarative and versioned (source of truth); GIFs regenerated only on master; example scripts idempotent; CI checks are grep-based and stable |
| III. Rust Best Practices | PASS | Zero Rust changes (FR-009) — no new unwraps, no new code paths; `cargo test`/`clippy`/`fmt`/`audit` gates unaffected |
| IV. Modularity (4-layer) | PASS | No layer touched; docs live in `docs/`, examples in `examples/`, tapes in `vhs/`, tests in `tests/` |
| V. Language | PASS | All new docs, scripts, tapes, and workflow names in English |

No violations. Complexity Tracking table not needed.

---

## Architecture

### Layer Responsibilities

No application-layer changes. The workstream adds a documentation layer to the repository:

```
README.md (entry)
  ├── Full Command Reference ──links──► docs/commands/envy-<cmd>.md (17 pages, shared template)
  ├── Examples ──links──► examples/{basic,team-sync,ci-cd,monorepo}/README.md + scripts
  └── Demos ──links──► docs/assets/*.gif (generated from vhs/*.tape in CI, master only)

tests/e2e_devops_scenarios.sh
  └── Scenario 15 — Documentation examples (runs each examples/* script, asserts outputs)

.github/workflows/ci.yml
  ├── build job ──adds──► FR-010 docs check (links resolve + template sections present)
  └── vhs-demos job (push to master only, vhs-action, auto-commit GIFs)
```

### Data Flow

```
vhs/*.tape ──(vhs-action, master push)──► docs/assets/*.gif ──(README link)──► user
examples/*/script.sh ──(Scenario 15, ENVY_BIN)──► assert_eq expected outputs ──► CI green
README table row ──(FR-010 grep check)──► docs/commands/envy-<cmd>.md exists + sections
```

---

## Repository Changes (no source code changes — docs/tests/CI only)

### 1. README.md — entry document (FR-001, FR-011)

- **Lines 318-336 (`Full Command Reference` table)**: convert the `Command` column cells into links to `docs/commands/envy-<cmd>.md` for all 17 commands (`init`, `set`, `get`, `list`, `rm`, `run`, `migrate`, `encrypt`, `decrypt`, `export`, `diff`, `status`, `rotate`, `scan`, `audit`, `hooks`, `completions`). Keep the existing descriptions and alias column. NOTE: locate the table by its `📋 Full Command Reference` heading, not by line number (lines shift after edits).
- **New section after the command reference**: `## Documentation` — links to `docs/commands/` index, `examples/`, and `docs/assets/` demo GIFs (when they exist post-merge).
- Keep Quickstart/Problem/Why Envy/How It Works/Installation/Roadmap sections unchanged.

### 2. docs/commands/ — 17 pages + template (FR-002, FR-003)

New files (naming: `envy-<cmd>.md` for stable sorting and unambiguous links):

```
docs/commands/
├── _template.md
├── envy-init.md
├── envy-set.md
├── envy-get.md
├── envy-list.md
├── envy-rm.md
├── envy-run.md
├── envy-migrate.md
├── envy-encrypt.md
├── envy-decrypt.md
├── envy-export.md
├── envy-diff.md
├── envy-status.md
├── envy-rotate.md
├── envy-scan.md
├── envy-audit.md
├── envy-hooks.md
└── envy-completions.md
```

**Shared template (`_template.md`) sections** (exact headings — the FR-010 grep check depends on them; all 6 are mandatory):

1. `# envy <cmd>` + one-line summary
2. `## What it does` — 1-2 sentences + when to use it
3. `## Aliases` — table of aliases if any (`ls`, `remove`, `unset`, `enc`, `dec`, `df`, `st`, `au`), or a single line "None"
4. `## Syntax & flags` — table extracted from `envy <cmd> --help` / README command reference
5. `## Examples` — code blocks with dummy values (e.g. `sk_test_...`), never real secrets
6. `## How it works` — security notes (encryption, memory zeroing, headless `ENVY_PASSPHRASE` behavior where relevant), exit codes (command-specific when ≠ generic)
7. `## Related commands` — cross-links (mandatory: every command has at least one related command)

**Content source of truth**: `src/cli/mod.rs` (Commands enum + clap args) for syntax/flags; `src/cli/commands.rs` + README exit-code table for behavior/exit codes; `src/crypto/*`, `src/core/*` for "How it works" security notes.

### 3. examples/ — 4 CI-verified scenarios (FR-004)

```
examples/
├── basic/
│   ├── README.md          # init → set → list → run walkthrough, prerequisites
│   ├── envy.toml.example  # commented manifest (project_id placeholder)
│   └── tutorial.sh        # idempotent: init, set 2 dummy secrets, list, run -- echo
├── team-sync/
│   ├── README.md          # Dev A → Dev B handoff: encrypt → copy envy.enc → decrypt → run
│   ├── dev_a.sh           # init, set secrets, encrypt headless (ENVY_PASSPHRASE_DEVELOPMENT)
│   └── dev_b.sh           # decrypt headless, assert secret present, run
├── ci-cd/
│   ├── README.md          # headless CI usage, exit codes, JSON gates
│   ├── workflow.yml       # copyable GitHub Actions workflow (encrypt-check + status gate)
│   └── headless.sh        # init, set, encrypt, status --format json, diff gates
└── monorepo/
    ├── README.md          # nested projects (feature 014)
    ├── apps/app-a/envy.toml.example
    ├── apps/app-b/envy.toml.example
    └── script.sh          # init both nested projects, verify independent secrets
```

**Script contract** (each script):
- Bash, `set -euo pipefail`, runnable via `ENVY_BIN=... ./tutorial.sh` (or `ENVY_BIN` default `envy` on PATH)
- Idempotent (safe to run twice; re-init handled via `envy init` in a fresh temp dir by the harness)
- Headless: exports **both** env vars before running envy — `ENVY_PASSPHRASE` (activates the keyring `ci_fallback`, which returns the deterministic zero key so the ephemeral vault works without an OS keyring daemon — required for `init`/`set`/`get`/`run`) AND `ENVY_PASSPHRASE_DEVELOPMENT` (the envelope passphrase used by `encrypt`/`decrypt` to seal/unseal `envy.enc`). Setting `ENVY_PASSPHRASE` makes the scripts work identically on the user's machine (no keyring) and in CI (where the `CI` var alone also triggers the fallback). In CI the vault uses the zero key → dummy values only.
- Dummy secret values only; never real credentials
- Prints a final success line the E2E can assert on (e.g. `basic: OK`)

### 4. tests/e2e_devops_scenarios.sh — Scenario 15 (FR-005)

New section following the existing style (`section()` + assertions with `assert_eq`/`assert_contains`/`assert_file_exists`):

- Header comment: add `#  15. Documentation examples (envy docs)` to the scenario list (lines 6-16)
- `Section 15 — Documentation examples`:
  1. For each of `examples/basic`, `examples/team-sync`, `examples/ci-cd`, `examples/monorepo`: create temp dir, run the scenario scripts via `"$ENVY"` with `ENVY_PASSPHRASE_DEVELOPMENT` set headlessly
  2. Assert: `basic` — `run` prints env var, `envy.toml` exists; `team-sync` — `envy.enc` exists after encrypt, decrypt round-trip succeeds; `ci-cd` — `status --format json` parses with `jq` and env is `in_sync`; `monorepo` — both nested projects initialise fresh (no committed `envy.toml` — manifests ship as `.example` to avoid `init` exit-3 conflicts) and secrets resolve independently
- Count toward TOTAL/PASS/FAIL like every other scenario

### 5. vhs/ — 3 hero tapes + CI job (FR-006, FR-007)

```
vhs/
├── quickstart.tape   # init → set (2 dummy) → list → run -- echo
├── team-sync.tape    # Dev A encrypt → show envy.enc → Dev B decrypt → run
└── ci-cd.tape        # headless encrypt + status --format json + diff gates
```

**Tape best practices** (VHS skill): `Output: docs/assets/<name>.gif` declared at file start; `Require` envy binary on PATH; `Env ENVY_PASSPHRASE_DEVELOPMENT=<dummy>` + `Env ENVY_PASSPHRASE=<dummy>` before commands for headless deterministic vault; explicit `Set TypingSpeed`, `Set FontSize`, `Set Width/Height`; `Sleep` after Enter; final `Sleep`; no real secrets.

**CI (`.github/workflows/ci.yml`)** — new job `vhs-demos`:
- `on: push: branches: [master]` only (FR-007 — fork PRs cannot push)
- `permissions: contents: write` (needed for auto-commit)
- Steps: checkout (with fetch-depth), build release binary, install VHS via `charmbracelet/vhs-action` (verify the latest stable version tag at implementation time — v2 expected), then git add/commit/push `docs/assets/*.gif` **only if changed** (no-op skip to avoid loops)
- Job must fail loudly on tape errors (default action behavior)

### 6. CI docs check (FR-010)

Add a lightweight step to the existing `build` job in `.github/workflows/ci.yml` (after E2E steps):

```yaml
- name: Docs check (links + template sections)
  shell: bash
  run: |
    # (a) every README command-table link resolves
    for f in $(grep -oE 'docs/commands/envy-[a-z]+\.md' README.md | sort -u); do
      test -f "$f" || { echo "missing: $f"; exit 1; }
    done
    # (b) every page has the required template sections (all 6 mandatory)
    for f in docs/commands/envy-*.md; do
      for s in '## What it does' '## Aliases' '## Syntax & flags' '## Examples' '## How it works' '## Related commands'; do
        grep -q "$s" "$f" || { echo "missing section '$s' in $f"; exit 1; }
      done
    done
```

Assert at least the 17 linked pages resolve and all pages carry the 6 mandatory headings (`## Aliases` may contain "None" for commands without aliases — the heading must still exist; `## Related commands` is mandatory because every command has at least one related command). The `_template.md` file itself is excluded from the section check (it is the source of truth).

### 7. docs/developer-guide.md — refresh (FR-008)

Update the project-structure tree (section 3, lines 71-110) to reflect current source:

- `src/cli/`: add `format.rs` (output formatting)
- `src/core/`: add `audit.rs` (audit log), `scan.rs` (vault-leak scanner)
- `src/crypto/`: add `strength.rs` (passphrase strength), `diceware.rs`
- `src/db/`: add `audit.rs` (audit_logs table), `sync_markers.rs` (V2 sync markers)
- Check the rest of the guide for stale references (e.g. `rotate_env` — verify it still exists in `src/core/sync.rs` at implementation time; section 7/8/10-11 headings) and update only what is factually outdated — no rewrite of unrelated content

---

## Testing Strategy

| Test | What it verifies | Where |
|------|------------------|-------|
| Scenario 15 — basic | init/set/list/run round-trip headless | tests/e2e_devops_scenarios.sh |
| Scenario 15 — team-sync | encrypt → decrypt round-trip, envy.enc exists | tests/e2e_devops_scenarios.sh |
| Scenario 15 — ci-cd | status JSON `in_sync` gate (jq), headless encrypt | tests/e2e_devops_scenarios.sh |
| Scenario 15 — monorepo | nested projects resolve independently | tests/e2e_devops_scenarios.sh |
| Docs check (FR-010) | README links resolve; pages have mandatory sections | .github/workflows/ci.yml build job |
| VHS job | 3 GIFs generated from tapes on master; no-op commit when unchanged | .github/workflows/ci.yml vhs-demos job |
| Existing suite | No regression: 14 scenarios / 114 assertions + cargo test + clippy + fmt + audit | unchanged |

Manual verification before PR:
- `cargo fmt --check && cargo clippy -- -D warnings` (must be unaffected)
- `ENVY_BIN=./target/debug/envy bash tests/e2e_devops_scenarios.sh` → ALL 14 + 15 scenarios pass
- `bash examples/basic/tutorial.sh` (with `ENVY_BIN`) and the other 3 scripts run clean in a temp dir
- `vhs vhs/quickstart.tape` locally produces a GIF (if VHS installed)

---

## Project Structure

### Documentation (this feature)

```text
specs/015-docs-examples-demos/
├── plan.md              # This file (/speckit.plan command output)
├── spec.md              # Feature specification (11 FRs, complete)
└── tasks.md             # Task breakdown (generated by /speckit.tasks)
```

Note: per the confirmed lightweight process, this feature intentionally has **no** research.md, data-model.md, contracts/, or quickstart.md — all decisions are already resolved in the spec and this plan.

### Repository Changes (new and modified files)

```text
README.md                              # MODIFIED — linked command table (FR-001), docs/examples/demos sections (FR-011)
docs/
├── commands/                          # NEW — 17 pages + _template.md (FR-002, FR-003)
│   ├── _template.md
│   └── envy-{init,set,get,list,rm,run,migrate,encrypt,decrypt,export,diff,status,rotate,scan,audit,hooks,completions}.md
├── developer-guide.md                 # MODIFIED — architecture tree refresh (FR-008)
└── assets/                            # NEW (CI-generated) — demo GIFs
examples/
├── basic/                             # NEW — README + envy.toml.example + tutorial.sh
├── team-sync/                         # NEW — README + dev_a.sh + dev_b.sh
├── ci-cd/                             # NEW — README + workflow.yml + headless.sh
└── monorepo/                          # NEW — README + apps/*/envy.toml.example + script.sh
vhs/
├── quickstart.tape                    # NEW
├── team-sync.tape                     # NEW
└── ci-cd.tape                         # NEW
tests/
└── e2e_devops_scenarios.sh            # MODIFIED — Scenario 15 (FR-005)
.github/
└── workflows/
    └── ci.yml                         # MODIFIED — FR-010 docs check + vhs-demos job (FR-007)
```

### Unchanged files

```text
src/**                    # No Rust changes (FR-009)
Cargo.toml / Cargo.lock   # No new dependencies
docs/assets/demo.gif      # Existing asset untouched
```

---

## Complexity Tracking

No constitution violations. Table not needed.
