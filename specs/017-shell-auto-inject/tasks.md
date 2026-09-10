# Tasks: Transparent Shell Auto-Injection (`017-shell-auto-inject`)

Conventions: `[x]` done + verified, `[ ]` pending. Unit tests live next to the
code per repo convention; keyring paths are `#[ignore]`d E2E in
`tests/cli_integration.rs`.

## Core — manifest flag

- [x] `Manifest.auto_inject: bool` with `#[serde(default)]` (old files → false)
- [x] `create_manifest_with_options(target, id, flag)`; `create_manifest` keeps
      signature (writes `false`)
- [x] `set_manifest_auto_inject(dir, flag)` — textual replace-or-insert,
      preserves comments/`rotation_reminder_days`
- [x] Round-trip + legacy-parse unit tests (`manifest.rs`, `shell.rs`)

## CLI — commands

- [x] `Init { --auto-inject }`, `Auto { on|off|status }` (`AutoAction`),
      `ShellInit [shell]`, `Hook { --shell, -e }` (`ShellKind`) in `Commands`
- [x] Early dispatch for vault-less `shell-init`/`hook`; shared lifecycle for
      `auto`; `cmd_init(auto_inject)` owns its vault lifecycle as before
- [x] `hook` infallible contract: always exit 0, unload-or-silence degradation

## CLI — hook core (`cli/shell.rs`)

- [x] `HookPlan` + `compute_hook_plan` (sort, validate, stale-unset diff)
- [x] Per-shell renderers (bash/zsh POSIX, fish, powershell, nushell JSON) +
      `render_unload` (empty stdout when nothing to unload)
- [x] `resolve_hook_env` (`--env` > `ENVY_ENV` > `development`, lowercased)
- [x] `hook_state_changed` gating for entry warnings (+ poisoning tests)
- [x] `shell_init_snippet` × 5 + direnv `.envrc` pattern in headers/docs

## CLI — one-step installer

- [x] `detect_shell`, `rc_file_for_shell` (bash/zsh/fish; `None` → manual),
      `oneliner_for_shell`, `hook_already_installed` (marker + shell-specific),
      `append_snippet_if_missing` (idempotent, preserves bytes)
- [x] `offer_hook_install` (default `N`, TTY guard, manual fallback, never
      fails `init`/`auto on`); wired into both
- [x] Idempotency + preservation unit tests

## Docs

- [x] `docs/commands/envy-{auto,shell-init,hook}.md` (template format)
- [x] `envy-init.md` (`--auto-inject` + prompt), `envy-run.md` (pointer),
      README quickstart + command table
- [x] `docs/developer-guide.md` module map (`shell.rs`, manifest fns)
- [x] This spec directory (`spec.md`, `plan.md`, this file)

## Verification

- [x] `cargo test shell::` + `cargo test manifest` green (no keyring)
- [x] Ignored E2E for `auto` round-trip, hook inject/unload/kill-switch,
      non-interactive `init --auto-inject`, `shell-init` output
- [x] Manual E2E on macOS zsh (opt-in → `y` install → restart → prefix-free
      `echo`/`python3` → unload on leave → single `.env` warning)
- [ ] `cargo test` (full) + `cargo clippy -- -D warnings` + `cargo audit`
      on a machine with the toolchain (no toolchain in this environment)
- [ ] `AGENTS.md` regeneration (file is auto-generated from plans; hand-edit
      deliberately skipped)
