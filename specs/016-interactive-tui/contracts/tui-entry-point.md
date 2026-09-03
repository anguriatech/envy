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

## TUI hotkey contract (FR-015, amended by FR-038/FR-040–FR-047)

| Key | Action |
|-----|--------|
| `Q` / `Ctrl+C` | Quit TUI (restore terminal, close vault) |
| `Esc` | Close popup/search; at top level shows a "Press Q to quit" hint — never quits |
| `B` | Toggle banner: full gradient ↔ compact single-line |
| `↑` / `↓` / `←` / `→` | Navigate sidebar (projects/envs) and table rows |
| `Tab` | Move focus: sidebar ↔ table |
| `Enter` | Activate sidebar selection (switch project/env) |
| `F` | Focus search box (live filter, `Esc` to close, `Enter` keeps filter) |
| `Space` | Toggle mask for selected row (re-masks on selection change) |
| `E` | Edit selected secret (popup; title shows the key name) |
| `N` | New secret (popup; `Enter` on the key field advances to the value field) |
| `D` | Delete selected secret (confirm popup showing key name and environment) |
| `S` | GitOps sync (env-var passphrases or per-env popup) |
| `L` / `U` | Lock / unlock vault (mutating keys are guarded while locked) |
| `T` / `G` / `Y` | Status / diff / import (import asks confirmation before overwriting) |
| `?` | Open help (scrollable with `↑`/`↓`/`j`/`k`) |
| paste | `Ctrl+Shift+V` / middle-click works in every text field (bracketed paste) |

### Popup mode (overrides)

| Key | Action |
|-----|--------|
| `Esc` | Cancel popup (also cancels an in-progress sync queue) |
| `Enter` | Confirm (edit/new/delete/passphrase/import) |
| `Ctrl+R` | Toggle masked ↔ revealed while typing a value (FR-012) |
| `↑` / `↓` / `j` / `k` | Scroll long text popups (help/diff/status) |
| any other | Ignored — global hotkeys (`L`, `S`, `Q`, …) are disabled while a popup is open |

### Text popups (FR-040)

Help, Diff, and Status popups are scrollable and clamp their height to 20 inner rows
(`popup_inner_height`/`popup_max_scroll` in `app.rs`). Titles carry a "↑↓ scroll" hint
when the content overflows.

## Status bar (FR-015, amended by FR-038)

`[Locked]|[Unlocked]` + active project `/` environment + latest status message + short
hotkey pointer (`? Help … Q Quit`). Error messages render highlighted (red) when the
terminal supports color; `NO_COLOR` disables this — FR-019.

## Silent execution guarantees (FR-002/FR-003)

- No TUI code path writes to stdout. TUI renders exclusively via ratatui's alternate screen buffer.
- `NO_COLOR` set → no color codes anywhere (banner plain, styles reset) — FR-019.
- Subcommand outputs are byte-identical; regression gate is the existing CLI test-suite + E2E.

## Exit codes

- TUI quit with `Q`: 0.
- Bare + non-TTY help: 0.
- TUI start failure (terminal setup): 1 (existing CliError exit-code convention).
- All subcommand exit codes: unchanged (see cli.md).
