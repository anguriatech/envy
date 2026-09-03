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

1. `envy` → banner renders with bluish-purple gradient; header shows `[Unlocked]` and active env.
2. `B` collapses/expands the banner.
3. `F` + type → visible search box and table filter update live; `Esc` closes search.
4. `Space` on a row → value reveals; arrow down → re-masks.
5. `N` → new secret popup (masked input, `Ctrl+R` reveals); `Enter` saves → `envy get KEY` outside shows it.
6. `E` / `D` round-trip; `D` asks confirmation.
7. `S` → syncs `envy.enc` (env-var passphrase if set, else masked popup per env); working indicator shows.
8. `L` → header `[Locked]`, table cleared, structure kept; `U` → reloaded.
9. `NO_COLOR=1 envy` → banner/UI plain, still usable.
10. `envy` with stdout piped → help on stderr, exit 0, no escapes (headless CI-safe).
11. Run `envy` from a nested project directory → `S` updates `envy.enc` beside discovered `envy.toml`, not nested cwd.
12. `P` opens searchable project selection; `X` deletes selected project only after exact-name confirmation; `?` opens help.
13. `↑↓` moves through all projects/tree entries; `Enter` or `→` expands/selects; `←` collapses.
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
