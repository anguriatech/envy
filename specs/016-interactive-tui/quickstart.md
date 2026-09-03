# Quickstart: Interactive TUI (016)

**Date**: 2026-08-10 | **Branch**: `016-interactive-tui`

## One-line summary

`envy` launched bare in a terminal opens a full-screen manager: gradient ENVY banner,
project/environment sidebar, searchable secrets table with masked values, edit/new/delete
popups, lock/unlock, and GitOps sync — silent everywhere else.

## Commands to verify (after implementation)

```bash
# 1. Build + quality gates
cargo build
cargo clippy -- -D warnings
cargo test

# 2. Silent guarantees (US2) — no TUI, no ANSI, exit 0
envy | head -c 0          # exits 0, nothing on stdout
envy --help | hexdump -C | grep -c '1b\[' || echo "no ANSI escapes"
echo "$?"                 # 0

# 3. Interactive session (US1) — requires a real TTY
envy                      # banner + sidebar + table; Q quits cleanly
```

## Manual smoke checklist (TTY only)

1. `envy` → three-panel console: tree | secrets | Details inspector (inspector hidden under 100 cols); bottom shows status row + contextual legend.
2. `B` collapses/expands the banner.
3. Tab between panels → legend row changes to the focused panel's keys.
4. Inspector: select project → sync summary (in sync / modified / never sealed); select environment → sync state + counts; select secret → key/value/updated + actions.
5. `:` → palette opens; type `seal` → matches; `Esc` closes; `Enter` executes "Project status" etc.
6. `S` → seal-preview confirmation listing environments with secret counts; `Esc` cancels.
7. `R` on an environment → rotate flow (current → new → confirm, masked, Ctrl+R reveals); wrong current passphrase fails without touching the artifact.
8. Seal an already-sealed environment with a different passphrase → error points to `R` (rotate) or `Y` (import), both recover in-TUI.
9. Secrets panel `Y` → "Copied 'KEY' — clipboard clears in 30s" in the status row; paste somewhere to verify; tree `Y` → import confirmation (unchanged).
10. `F` + type → visible search box and live filter; `Enter` closes the line and the filter stays visible in the panel title.
11. `Space` on a row → value reveals; arrow down → re-masks.
12. `N` → new secret popup; `Enter` on the key field moves to the value field; `Enter` saves.
13. `L` → `[Locked]`, keys guarded with "Vault locked — press U to unlock"; `U` → reloaded.
14. `Esc` on the main screen shows the "Press Q to quit" hint and does NOT exit; `Q`/`Ctrl+C` exits.
15. `?` opens scrollable grouped help; `G`/`T` popups scroll if long.
16. Paste (`Ctrl+Shift+V`) into the New/Edit/passphrase/search fields.
17. `NO_COLOR=1 envy` → plain but fully usable; errors still readable.
18. `envy` with stdout piped → help on stderr, exit 0, no escapes (headless CI-safe).
19. Run `envy` from a nested project directory → `S` updates `envy.enc` beside discovered `envy.toml`, not nested cwd.
20. `P` opens searchable project selection; `X` deletes selected project only after exact-name confirmation.
14. `T` shows project status, `G` shows active-environment diff, `Y` imports/unseals active environment.
15. Long project lists scroll while keeping highlighted project visible; `P` picker scrolls all matches.
16. Footer shows primary controls only; press `?` for grouped complete help.

## Environment variables

- `NO_COLOR` — disables all color in the TUI.
- `ENVY_PASSPHRASE` / `ENVY_PASSPHRASE_<ENV>` — headless sync passphrases (no popup).

## Files

- Plan: `specs/016-interactive-tui/plan.md`
- State/design: `specs/016-interactive-tui/{research,data-model}.md`
- Contracts: `specs/016-interactive-tui/contracts/tui-entry-point.md`
