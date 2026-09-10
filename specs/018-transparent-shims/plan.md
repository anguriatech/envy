# Plan: Transparent Command Shims (`018-transparent-shims`)

## Context

Supersedes the 017 hook mechanism (see `spec.md` ADR-001): same motive
(prefix-free runs) and same opt-in flag, different mechanism — PATH shims
with `run`-grade scoping so the parent shell never holds secrets.
Linux/macOS/Windows from v1 via dumb shims + all logic in Rust.

## Decisions

1. **Dumb shims, smart `exec`.** Shim files contain no logic (POSIX `sh` /
   Windows `.cmd` twins calling `envy exec -- <name>`); resolution, manifest
   walk, opt-in check, injection, and spawning all live in `cli/shim.rs`.
   One implementation to test, per-OS file generation only.
2. **Reuse, don't duplicate.** Inject path delegates to existing
   `cmd_run` (single spawn/audit/exit-code semantics); `auto_inject` flag +
   manifest helpers + `auto on/off/status` + `shell-init` command shape kept.
3. **Vault-free admin paths.** `reshim`/`shim`/`doctor`/`shell-init` never open
   the vault (fs + manifest reads only) — usable without a keyring daemon,
   which also enables hermetic non-ignored E2E (redirected HOME).
4. **Loud failures only.** `exec`/`reshim` surface every failure on stderr
   with table-consistent codes (127 not-found, 4 vault, 2 bad input, 1
   doctor findings / missing manifest/shim). The only exit-0-always command
   was the hook, which is deleted.
5. **Windows as twin files.** `.cmd` with CRLF (runs under cmd.exe and
   PowerShell, no ExecutionPolicy friction); deliberately NO extensionless
   files on Windows (a bare file would shadow `.cmd` under CreateProcess
   lookup). Unix gets extensionless + `0o755` (existing `set_executable`
   helper, already a Windows no-op).

## Layer mapping (Constitution Principle IV)

- `cli/shim.rs` (new): detectors, shim read/write/prune/list, PATH
  resolution, `cmd_reshim` / `cmd_shim` / `cmd_exec` / `cmd_doctor` (all
  `pub(super)`; `ShellKind`-free).
- `cli/shell.rs` (slimmed): `ShellKind`, `detect_shell`, PATH-line snippets,
  `cmd_shell_init`, opt-in hint helper. Hook plan/render/escaping and the rc
  installer are deleted with their tests.
- `cli/mod.rs`: drop `Hook`; add `Reshim`, `Shim` (+`ShimAction`), hidden
  `Exec`, `Doctor`. `Shim`/`Exec`/`Doctor` dispatch early (no manifest or
  vault needed); `Reshim` dispatches post-manifest, pre-vault (like `Hooks`).
- `cli/commands.rs`: `cmd_init`/`--auto-inject` output reworked (no offer);
  `audit_best_effort` stays private — `exec` reuses `cmd_run`, so no new
  audit path is needed.
- `cli/error.rs`: one additive variant `InvalidShimName` → exit 2.
- `core/manifest.rs`: template comment repointed at shims; functions unchanged.

## Risks

- **PATH-order bypass** (managers prepending): mitigated by installer
  copy (line LAST), `doctor` C2 check, and docs — residual risk accepted, ADR.
- **Detector false comfort** (`docker build` exclusion documented; manual add
  allowed): accepted long-tail tradeoff, ADR.
- **`exec` double vault-open** when invoked through `run` or nested shims
  (npm→npx): correct, millisecond overhead, no dedup in v1.
- **No toolchain in this environment**: `cargo test/clippy/fmt/audit` must run
  on the maintainer machine + CI; code reviewed line-by-line here instead.
