# Implementation Plan: Interactive TUI (ratatui + crossterm)

**Branch**: `016-interactive-tui` | **Date**: 2026-08-10 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/016-interactive-tui/spec.md`

## Summary

Add a full-screen TUI to Envy. Bare `envy` in an interactive terminal launches a ratatui
interface: pixel-art "ENVY" banner with bluish-purple gradient (compactable), project/
  environment tree sidebar, searchable project picker, secrets table with masked-by-default values, live search, edit/new/
  delete popups, safe project deletion, contextual help, status/diff/import actions, real lock/unlock against the vault, and GitOps sync with per-env passphrase
resolution. Everything else (`envy <subcommand>`, piped output, `run`) stays byte-identical
and silent. Decrypted values live only in `Zeroizing` buffers; terminal and vault are
restored/closed on every exit path. Artifact paths follow the discovered manifest root;
sync markers are committed only after atomic artifact writes. New deps: `ratatui 0.29` with
`crossterm 0.28` and `console 0.15` (direct, for color-depth detection).

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.85 (project standard)
**Primary Dependencies**: `ratatui 0.29` (new), `crossterm 0.28` (new direct), `console 0.15` (new direct; already in tree), `clap 4` (derive), `zeroize` (existing, `derive`)
**Storage**: SQLCipher vault via `rusqlite` (read/write through `core/` only); no schema changes
**Testing**: `cargo test` — unit tests on pure TUI state (gradient, filter, mask, lock transitions, CRUD round-trip), integration test for bare-piped help; existing suite must stay green
**Target Platform**: Linux/macOS/Windows terminals (crossterm cross-platform); TTY only
**Project Type**: CLI (additive interactive surface)
**Performance Goals**: TUI usable in <1 s after vault unlock; event loop poll 100 ms; sync freeze ≤ ~500 ms (Argon2id) with working indicator
**Constraints**: zero secret leakage (Principle I), byte-identical subcommand output, `NO_COLOR`, no async runtime, keyboard-only
**Scale/Scope**: small state (projects/envs/secrets counts are vault-sized, typically <1k rows); table scrolls

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Security by Default | ✅ PASS | Values in `Zeroizing<String>`; masked by default; lock closes vault + drops key; no secret ever written or logged; TUI never writes to stdout |
| II. Determinism | ✅ PASS | Subcommand output byte-identical (FR-003); bare-non-TTY help is stable, documented, exit 0; banner gradient deterministic (fixed stops, lerp) |
| III. Rust Best Practices | ✅ PASS | Typed `Result` errors via existing `CliError`; no `.unwrap()`/`.expect()` without inline justification; unit + integration tests; clippy `-D warnings` gate |
| IV. Modularity | ✅ PASS | `src/cli/tui/` lives in the CLI layer; delegates all vault/crypto work to `core` (`set_secret`, `list_secrets_with_values`, `seal_artifact`, …); only the two permitted exceptions (`Vault::open`, `get_or_create_master_key`) |
| V. Language | ✅ PASS | All identifiers/comments/docs/messages in English |

*No violations → Complexity Tracking table not needed.*

## Project Structure

### Documentation (this feature)

```text
specs/016-interactive-tui/
├── plan.md              # This file
├── research.md          # R-001..R-009 (stack, colors, dispatch, zeroize, sync)
├── data-model.md        # TUI state entities + VaultState machine
├── quickstart.md        # Verification commands + manual smoke checklist
├── contracts/
│   └── tui-entry-point.md  # Bare dispatch + hotkey contract + silent guarantees
├── checklists/
│   └── requirements.md  # Spec quality checklist (all pass)
└── tasks.md             # Created by /speckit.tasks (not by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── cli/
│   ├── mod.rs           # Cli.command → Option<Commands>; bare dispatch (tui vs help)
│   ├── commands.rs      # unchanged (dialoguer flows intact)
│   ├── error.rs         # unchanged (exit-code mappers reused)
│   ├── format.rs        # unchanged
│   └── tui/             # NEW — interactive TUI module
│       ├── mod.rs       # pub(super) fn run() -> Result<(), CliError>: terminal setup,
│       │                #   guard (Drop: restore + vault close), event loop
│       ├── app.rs       # pure App state: VaultState, projects/envs cache, SecretEntry
│       │                #   (Zeroizing), SearchQuery, PopupState, SyncState, BannerState;
│       │                #   handle_key(KeyKind) methods — no crossterm/ratatui types
│       ├── theme.rs     # palette, gradient stops, lerp + 256 quantization,
│       │                #   color-depth detection via console, NO_COLOR handling
│       ├── banner.rs    # "ENVY" block-letter ASCII art (const lines) + per-line color
│       ├── widgets.rs   # line editor (masked/revealed input), centered popup layout,
│       │                #   confirm popup
│       ├── ops.rs       # thin bridge to core: load_projects/envs/secrets, set/delete,
│       │                #   sync loop (passphrase resolve → seal → verify → write),
│       │                #   lock/unlock (Vault::open/close + get_or_create_master_key)
│       └── ui.rs        # Frame → widgets rendering (header/banner, sidebar, table,
│                        #   visible search box, metadata, status bar, popups)
tests/
├── cli_integration.rs   # + bare `envy` piped → help on stderr, exit 0, no ANSI
└── sync_artifact.rs     # unchanged (core sync untouched)
```

**Structure Decision**: single Rust binary crate, TUI as a submodule of the CLI layer
(matches the existing `commands.rs` pattern and Constitution IV layer rules). State split
from rendering (`app.rs` pure → unit-testable headlessly; `ui.rs` thin renderer).

## Implementation Steps

### Step 1 — Dependency & dispatch groundwork
- `Cargo.toml`: add `ratatui = "0.29"`, `crossterm = "0.28"`, `console = "0.15"`.
- `cli/mod.rs`: `command: Option<Commands>`; in `run()`: `None` + `stdout().is_terminal()`
  → `tui::run()`; `None` + piped → `Cli::command().print_help()` (stderr) + return 0.
- Integration test: bare piped → exit 0, stderr help, stdout empty/no-ANSI (FR-002/SC-002).
- **Verify**: `cargo test`, `cargo clippy -- -D warnings`, existing E2E output byte-identical.

### Step 2 — Terminal lifecycle + app skeleton
- `tui/mod.rs`: `ratatui::init()`/restore with Drop guard (panic hook included, FR-017);
  event loop `poll(100ms)` + `read()`; quit on `Q` → guard closes vault (FR-018).
- `tui/app.rs`: `VaultState`, cached projects/envs, `handle_key` skeleton, empty-vault and
  empty-env empty states (US3 edge cases).
- Unit tests: lock/unlock transitions with temp vault + `TEST_MASTER_KEY` (FR-013, FR-020);
  vault-close-on-drop assertion.

### Step 3 — Banner + theme
- `tui/theme.rs` + `tui/banner.rs`: const "ENVY" block ASCII; vertical gradient lerp over
  stops `#8A2BE2 → #7B68EE → #9370DB → #1A0933 → #0D0221`; per-line colors; TrueColor /
  256-quantized / plain per `console::Term::color_depth()`; `NO_COLOR` → plain (FR-004,
  FR-019); `B` toggle compact (FR-005). Header line: `[Locked]/[Unlocked]` + active env (FR-006).
- Unit tests: lerp math, stop ordering, quantization, NO_COLOR path (FR-020).

### Step 4 — Sidebar + table + search
- `tui/ui.rs`: layout header / compact active-project sidebar / table (key, masked value, updated)
  / status bar; keyboard nav (`Tab`, arrows, `Enter`); `F` search box live filter (FR-007,
  FR-008, FR-010); masked by default, `Space` per-row reveal, re-mask on move (FR-009,
  FR-011); "no matches" state; status bar hints (FR-015); table scroll for long lists.
- Unit tests: filter logic, mask/re-mask-on-move state transitions (FR-020).

### Step 5 — CRUD popups
- `tui/widgets.rs`: line editor (printable chars, backspace, Esc cancel, Enter confirm),
  masked-by-default + `Ctrl+R` reveal (FR-012), centered popups for secret deletion,
  project picker, help, and exact-name project deletion.
- `tui/ops.rs`: new/edit/delete via `core::set_secret` / `core::delete_secret`; key
  validation errors (`InvalidSecretKey`, empty value) → status-bar error; refresh table
  after mutation; popup open → global hotkeys ignored (edge case).
- Unit tests: CRUD round-trip against temp vault; popup state transitions; error surfacing.

### Step 6 — Lock/unlock + sync
- `tui/ops.rs`: `L` → `vault.close()` + drop key + clear values (keep structure); `U` →
  `get_or_create_master_key()` + `Vault::open` + reload (FR-013; keyring failure → status
  bar, stays locked). Header updates.
- `S` → resolve artifact beside discovered manifest; sync loop (research R-006): per env → passphrase from `ENVY_PASSPHRASE_<ENV>`/
  `ENVY_PASSPHRASE` else masked popup; skip 0-secret envs; verify existing envelope
  (`check_envelope_passphrase`); `core::seal_artifact` + `core::write_artifact`; working
  indicator in status bar (FR-014). Mark sync status only after atomic artifact write succeeds.
  Errors (mismatch → hint `envy rotate`) → status bar.
- Unit tests: lock/unlock transitions (values cleared, structure kept, reload works);
  passphrase-resolution ordering (env var wins; popup path state); sync success against
  temp env + artifact path.

### Step 7 — Docs + gates
- README: "Interactive TUI" section (launch conditions, hotkey table, silent rules) (FR-021).
- Feature documentation lives under `specs/016-interactive-tui/` only (FR-021 amended
  2026-09-03: the `docs/features/` copy was removed as duplication).
- Full run: `cargo test`, `cargo clippy -- -D warnings`, `cargo audit`; manual smoke
  checklist from `quickstart.md` on a TTY.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Bare-invocation behavior change breaks scripts | FR-003 regression gate: existing test-suite + E2E run unchanged; bare non-TTY explicitly exit 0 + stderr help |
| TUI freezes during Argon2id sync | Working indicator (FR-014); sync is user-triggered; async deferred (Non-Goals) |
| Color output wrong on exotic terminals | `console::Term::color_depth()` detection + explicit 256 quantization + plain fallback; NO_COLOR |
| Terminal left raw after panic | `ratatui::init()` panic-hook restore + Drop guard on all exit paths (FR-017) |
| Per-env passphrases block headless TUI sync | Env-var-first resolution (R-006) keeps CI/headless flows silent |
| ratatui API drift vs plan | Pin `ratatui = "0.29"`; use the stable terminal/event APIs supported by MSRV 1.85 |
| TUI writes artifact from wrong directory | Resolve artifact beside `envy.toml` with `core::find_manifest`; test nested cwd launch |
| Failed artifact write creates false healthy status | Seal without marker, write atomically, then commit marker; test write failure path |
| Rendered plaintext survives beyond draw | Borrow revealed text directly from `Zeroizing<String>` state; avoid intermediate plaintext allocations |
| Destructive project deletion surprises users | Show counts and require exact project-name confirmation; test cascade and cancellation |
| Tree navigation changes context unexpectedly | Cursor movement stays pure; only Enter/Right selects and loads project/environment |
| Status/diff actions leak plaintext | Status contains names/counts only; diff contains keys/change types, never values |
| Long project lists hide current selection | Use stateful ratatui lists for tree and picker; cursor movement remains visible |
| Dense controls overwhelm footer | Footer shows only primary navigation and `?`; full bindings live in grouped help popup |
