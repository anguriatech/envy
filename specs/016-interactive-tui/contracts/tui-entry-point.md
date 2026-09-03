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

## TUI hotkey contract (FR-015, amended by FR-038/FR-040–FR-059)

Panel-scoped keys act on the focused panel; the bottom legend row always
declares the focused panel's keys (FR-052). Every action is also reachable
through the `:` command palette (FR-053).

| Key | Panel | Action |
|-----|-------|--------|
| `↑`/`↓`/`j`/`k` | both | Move selection |
| `Enter` / `→` | tree | Expand project / select environment |
| `←` | tree | Collapse project |
| `Tab` | both | Switch tree ↔ secrets |
| `Space` | secrets | Reveal selected value (re-masks on move) |
| `Y` | secrets | Copy value to clipboard (clears 30s after last copy) |
| `N` / `E` / `D` | secrets | New / edit / delete secret |
| `F` | secrets | Filter keys (`Enter` keeps filter, `Esc` closes) |
| `S` | both | Seal project (preview confirmation first) |
| `T` | tree | Project status |
| `G` | tree | Diff active environment against `envy.enc` |
| `Y` | tree | Import active environment from `envy.enc` (confirms) |
| `R` | tree | Rotate environment passphrase (current → new → confirm) |
| `X` | tree | Delete project (exact-name confirmation) |
| `P` | both | Project picker (searchable) |
| `L` / `U` | both | Lock / unlock vault |
| `:` | both | Command palette (searchable, every action) |
| `?` | both | Full help overlay (scrollable) |
| `B` | both | Toggle banner |
| `Q` / `Ctrl+C` | both | Quit (restore terminal, close vault) |
| `Esc` | both | Close popup/search; at top level shows a quit hint — never quits |
| paste | inputs | Bracketed paste in every text field |

Destructive seal mismatches recover in-TUI (FR-056): the error points to
`R` (rotate) or `Y` (import) instead of `envy rotate` on the CLI.

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
