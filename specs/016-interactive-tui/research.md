# Research: Interactive TUI (016)

**Date**: 2026-08-10 | **Branch**: `016-interactive-tui`
**Input**: `specs/016-interactive-tui/spec.md`

## R-001 — TUI stack: ratatui version

- **Decision**: `ratatui 0.29` + direct `crossterm 0.28`.
- **Rationale**: ratatui 0.30 releases currently require Rust 1.86+, while project MSRV is 1.85. ratatui 0.29 supports Rust 1.74 and crossterm 0.28, so it preserves the project MSRV. The duplicate crossterm version is preferable to violating MSRV.
- **Alternatives considered**: ratatui 0.30 + crossterm 0.29 (rejected: exceeds MSRV); ratatui 0.29 with an indirect crossterm dependency only (rejected: TUI event API should be an explicit direct dependency).

## R-002 — Terminal capability detection for the banner gradient

- **Decision**: use `console::Term::color_depth()` — `console 0.15.11` is already in the tree (transitive via dialoguer); add it as a **direct** dependency. Map `ColorDepth` to: `TrueColor` → `ratatui::style::Color::Rgb`; `Color256` → quantize the lerped RGB to the 256-color cube (levels 0/95/135/175/215/255); `Color16`/`None` → plain (no color).
- **Rationale**: ratatui does NOT auto-map RGB to 256-color (FR-004 requires explicit fallback). `console` detects capabilities from TERM/COLORTERM reliably and is already vendored.
- **Alternatives considered**: parsing COLORTERM/TERM manually (fragile, duplicates console's logic); `crossterm` has no color-depth API.

## R-003 — Interactive vs silent dispatch

- **Decision**: change `Cli.command` to `Option<Commands>` (`clap` derive makes the subcommand optional). In `run()`: `None` + `std::io::IsTerminal::is_terminal(stdout)` → `tui::run()`; `None` + non-TTY → `Cli::command().print_help()` to **stderr**, exit **0** (deliberate deviation from clap's default exit 2 — confirmed with user). `Some(cmd)` → existing dispatch untouched.
- **Rationale**: `IsTerminal` pattern already used in `commands.rs:1164` (rotate) and `:1804` (NO_COLOR). Guarantees zero TUI/banner leakage into piped/CI output (US2).
- **Alternatives considered**: `arg_required_else_help` (exit 2, stderr write conflicts with FR-002); env-var gate (rejected: pollutes global namespace).

## R-004 — Event loop and terminal lifecycle

- **Decision**: `ratatui::init()` / `ratatui::restore()` (0.30 built-ins: alternate screen + raw mode + panic hook that restores the terminal) wrapped in a guard struct with `Drop` so restore runs on ALL exit paths (FR-017). Event loop: `crossterm::event::poll(Duration::from_millis(100))` + `crossterm::event::read()`.
- **Rationale**: `DefaultTerminal` + `terminal.draw(...)` per frame; 100 ms poll keeps redraws cheap (table is small) and gives the working indicator a natural tick.
- **Alternatives considered**: crossterm `EventStream` (needs async — excluded by Non-Goals); manual setup (duplicates ratatui's panic-hook handling).

## R-005 — Zeroization (Principle I)

- **Decision**: all decrypted secret values and edit-buffer contents live in `zeroize::Zeroizing<String>`. Master key stays in the `Zeroizing<[u8; 32]>` returned by `crypto::get_or_create_master_key()` (keyring.rs:93) and is dropped on lock (FR-013). On TUI exit: `vault.close()` (db/mod.rs:151) + state drop. Values are re-masked on selection change (FR-011); only masked text reaches the draw buffer.
- **Rationale**: `Zeroizing` is already a project dependency (`zeroize` with `derive`); `resolve_passphrase` already returns `Zeroizing<String>` (commands.rs:649) — same pattern.
- **Alternatives considered**: custom drop impls (rejected: Zeroizing is sufficient and audited).

## R-006 — GitOps sync inside the TUI (FR-014)

- **Decision**: replicate the per-environment sealing loop of `cmd_encrypt` (commands.rs:706) using core functions only: `core::read_artifact`, `core::seal_artifact(vault, key, project_id, passphrase, envs)`, `core::check_envelope_passphrase`, `core::write_artifact`. Passphrase resolution per env: (a) `ENVY_PASSPHRASE_<ENV>` then `ENVY_PASSPHRASE` env vars — headless, no UI; (b) else masked passphrase popup (line editor, zeroized). Skip empty environments (0 secrets) as the CLI does. Show working indicator in the status bar (sync = Argon2id seal ~100–500 ms).
- **Rationale**: core `seal_artifact` takes ONE passphrase per call and the CLI loops per env with per-env verification; TUI must mirror that loop. `check_envelope_passphrase` guards mismatched envelopes with the same error the CLI reports (verify-or-fail, spec 013).
- **Alternatives considered**: call `cmd_encrypt` directly (rejected: it prompts via dialoguer and prints to stdout — breaks the TUI); copy the loop logic into tui (rejected: layer rules — tui is cli layer, must delegate to core).

## R-007 — Lock/unlock semantics (FR-013)

- **Decision**: state keeps a cached non-secret structure (projects + environments, names only). `L`: `vault.close()` + drop master key + clear `Zeroizing` values + clear table. `U`: `get_or_create_master_key()` + `Vault::open(&cli::vault_path(), key)` + reload secrets. Header reflects `[Locked]`/`[Unlocked]`.
- **Rationale**: `vault_path()` is `pub(super)` in cli/mod.rs:325 — accessible from the tui module. Keyring failures at unlock surface as a status-bar error, TUI stays locked (edge case).
- **Alternatives considered**: full state wipe on lock (rejected: empty sidebar after unlock is worse UX; structure is not secret).

## R-008 — Testability of TUI state

- **Decision**: `app.rs` holds pure state (no crossterm/ratatui types — selection indices, search query, mask flags, `Zeroizing` values) with `handle_key(key: KeyKind)`-style methods; `ui.rs` maps state → widgets. Unit tests cover: gradient interpolation + quantization, search filter, mask/unmask/re-mask-on-move, lock/unlock transitions, CRUD round-trips against a temp vault (`tempfile` + `TEST_MASTER_KEY` — existing pattern, commands.rs:2405).
- **Rationale**: full-screen TUI cannot run headlessly in CI; splitting state from rendering makes FR-020 testable without a TTY.
- **Alternatives considered**: ratatui snapshot testing (`ratatui::TestBackend`) for a smoke draw test (possible bonus — render one frame to a buffer and assert no panic); keep in plan as optional.

## R-009 — Dependencies delta

- **New direct deps**: `ratatui = "0.30"`, `console = "0.15"` (already in lock, direct for color depth). No new transitive versions introduced.
- **Unchanged**: clap, zeroize, rusqlite, keyring, dialoguer (subcommand flows untouched), serde_json, comfy-table.
