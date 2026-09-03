# Contract: TUI Entry Point & Dispatch

**Feature**: 016-interactive-tui
**Date**: 2026-08-10
**Stability**: Draft — amends `004-cli-interface/contracts/cli.md`

## Bare-invocation dispatch (new behavior)

`Cli.command` changes from `Commands` (required) to `Option<Commands>`.

| Invocation | stdout is TTY | Behavior |
|------------|---------------|----------|
| `envy` (bare) | yes | Launch full-screen TUI (`cli::tui::run()`), exit code 0 on `Q` |
| `envy` (bare) | no (piped/redirected) | Print help to **stderr**, exit code **0**, zero ANSI escapes |
| `envy <subcommand> ...` | any | Existing dispatch, byte-identical output (FR-003) |

Decisions confirmed with user: exit 0 (deliberate deviation from clap's default exit 2);
no env-var gate.

## TUI hotkey contract (FR-015)

| Key | Action |
|-----|--------|
| `Q` / `Esc` (table mode) | Quit TUI (restore terminal, close vault) |
| `B` | Toggle banner: full gradient ↔ compact single-line |
| `↑` / `↓` / `←` / `→` | Navigate sidebar (projects/envs) and table rows |
| `Tab` | Move focus: sidebar ↔ table |
| `Enter` | Activate sidebar selection (switch project/env) |
| `F` | Focus search box (live filter, `Esc` to close) |
| `Space` | Toggle mask for selected row (re-masks on selection change) |
| `E` | Edit selected secret (popup) |
| `N` | New secret (popup) |
| `D` | Delete selected secret (confirm popup) |
| `S` | GitOps sync (env-var passphrases or per-env popup) |
| `L` | Lock vault |
| `U` | Unlock vault |

### Popup mode (overrides)

| Key | Action |
|-----|--------|
| `Esc` | Cancel popup |
| `Enter` | Confirm (edit/new/delete/passphrase) |
| `Ctrl+R` | Toggle masked ↔ revealed while typing a value (FR-012) |
| any other | Ignored — global hotkeys (`L`, `S`, `Q`, …) are disabled while a popup is open |

## Status bar (FR-015)

`[Q] Quit [B] Banner [F] Find [SPACE] Unmask [E] Edit [N] New [D] Delete [S] Sync [L] Lock [U] Unlock`
— compacted to fit terminal width; right side shows vault state `[Locked]`/`[Unlocked]` and working indicator during sync.

## Silent execution guarantees (FR-002/FR-003)

- No TUI code path writes to stdout. TUI renders exclusively via ratatui's alternate screen buffer.
- `NO_COLOR` set → no color codes anywhere (banner plain, styles reset) — FR-019.
- Subcommand outputs are byte-identical; regression gate is the existing CLI test-suite + E2E.

## Exit codes

- TUI quit with `Q`: 0.
- Bare + non-TTY help: 0.
- TUI start failure (terminal setup): 1 (existing CliError exit-code convention).
- All subcommand exit codes: unchanged (see cli.md).
