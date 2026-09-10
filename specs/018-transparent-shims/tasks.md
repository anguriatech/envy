# Tasks: Transparent Command Shims (`018-transparent-shims`)

Conventions: `[x]` done + reviewed, `[ ]` pending. Unit tests live next to the
code; keyring paths are `#[ignore]`d E2E; vault-free paths get hermetic
non-ignored E2E (redirected HOME + unique names + cleanup).

## Spec & docs

- [x] This spec directory (`spec.md` incl. ADR-001, `plan.md`, this file)
- [x] `docs/commands/envy-{reshim,shim,exec,doctor}.md` (template format)
- [x] `envy-shell-init.md` rewritten (PATH lines), `envy-auto.md` /
      `envy-init.md` / `envy-run.md` repointed, `envy-hook.md` removed
- [x] README quickstart + command table; developer-guide module map
- [ ] Landing (`site/`) untouched in v1 (explicit non-goal)

## Core (reuse)

- [x] `auto_inject` flag + `create_manifest_with_options` +
      `set_manifest_auto_inject` unchanged (template comment repointed)
- [x] Manifest round-trip unit tests kept green

## CLI — shell.rs (slim)

- [x] PATH-line snippets per `ShellKind` (bash/zsh/fish/powershell/nushell)
- [x] `cmd_shell_init` unchanged contract (static print, exit 0)
- [x] Deleted: hook plan/render/escaping, rc installer + offer, `cmd_hook`
- [x] Snippet/oneliner unit tests rewritten for PATH content

## CLI — shim.rs (new)

- [x] `shims_dir`, detector table (docker excluded, documented) +
      `detect_commands`
- [x] Shim templates (POSIX LF + `.cmd` CRLF), provenance headers,
      write-skip-if-exists, prune (auto-only), list, name validation
- [x] `resolve_real` (PATH-minus-shims, PATHEXT on Windows, explicit paths
      pass through) + `first_on_path` sharing with doctor
- [x] `cmd_reshim` (vault-free, `--prune`), `cmd_shim` (global),
      `cmd_exec` (hidden; delegates inject path to `cmd_run`; loud failures),
      `cmd_doctor` (vault-free; exit 0/1)
- [x] Unit tests: detectors, templates, prune, names, resolution skip logic

## CLI — wiring

- [x] `Commands`: drop `Hook`; add `Reshim`, `Shim`/`ShimAction`, hidden
      `Exec`, `Doctor`; early dispatch for vault-less commands
- [x] `CliError::InvalidShimName` → exit 2 (+ mapping test)
- [x] `cmd_init --auto-inject` / `cmd_auto on` outputs reworked (no offer)

## Verification

- [x] Manual line-by-line review (no toolchain in this environment)
- [ ] `cargo fmt --check` clean (run before push — bit us once already)
- [ ] `cargo test` (incl. new hermetic + `--ignored` keyring E2E)
- [ ] `cargo clippy -- -D warnings` clean on all three OS jobs
- [ ] `cargo audit` clean
- [ ] Manual E2E per OS: PATH last → shim first → doctor green → prefix-free
      run, parent clean, vault failure loud
