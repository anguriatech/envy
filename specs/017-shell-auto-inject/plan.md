# Plan: Transparent Shell Auto-Injection (`017-shell-auto-inject`)

## Context

`envy run -- cmd` injects secrets only into one child process. Humans and AI
agents keep running bare commands (`npm run dev`, `npx expo …`), so vault
secrets are silently ignored in favour of legacy `.env` files. Intercepting
arbitrary spawns is impossible on Unix — the fix is parent-shell cooperation:
a one-time hook (direnv/mise pattern) plus a per-project opt-in flag.

## Decisions

1. **Opt-in per project, setup per machine.** `auto_inject: bool` in
   `envy.toml` (default `false`, serde default keeps old manifests parseable).
   The hook is useless without the one-time `shell-init` line, and `init` never
   touches the rc file without an explicit `y` (default `N`).
2. **New module `src/cli/shell.rs`.** Keeps hook rendering, snippets, and the
   installer out of the already-large `commands.rs`. Presentation of hook
   output lives here rather than `format.rs`: it is shell-executable code
   (machine interface, like `completions`), not a `--format` variant.
3. **`envy hook` owns an infallible contract.** Handled before vault-dir
   creation in `run()`; every failure → unload-cleanup or silence, exit 0.
   Normal `auto`/`init` flows keep the shared vault lifecycle.
4. **State-change gating for warnings.** `chpwd` + `precmd` both fire per `cd`,
   so `.env`/skipped-key warnings are gated on `hook_state_changed`
   (injected-set transition), not per invocation.
5. **No new dependencies, exit codes, or error variants.** `dialoguer::Confirm`
   and `dirs::home_dir` were already in the tree. Invalid shell identifiers are
   skipped (validated on both vault and `$__ENVY_KEYS` sides).

## Layer mapping (Constitution Principle IV)

- `core/manifest.rs`: `Manifest.auto_inject`, `create_manifest_with_options`,
  `set_manifest_auto_inject` (text edit preserving comments/settings).
- `cli/shell.rs`: `ShellKind`, `HookPlan` + `compute/render`, snippets,
  `offer_hook_install`, `cmd_auto` / `cmd_hook` / `cmd_shell_init`.
- `cli/mod.rs`: `AutoAction`, `Init { auto_inject }`, `Auto`, `ShellInit`,
  `Hook` variants + early dispatch for the vault-less two.
- `cli/commands.rs`: `cmd_init(auto_inject)` calls the offer best-effort.
- Only sanctioned infra exceptions used: `Vault::open`,
  `crypto::get_or_create_master_key`.

## Risks

- **Prompt latency**: hook opens SQLCipher vault + keyring per prompt (~ms).
  Accepted for v1; `ENVY_AUTO_INJECT=0` is the escape hatch.
- **zsh/fish/pwsh/nu snippet drift**: five snippets maintained by hand + unit
  tests asserting structure (marker, hook call, kill-switch mention).
- **Eval safety**: mitigated by key validation + per-shell value escaping +
  poisoning tests; values travel as plain `String` only at the render boundary
  (same precedent as `export --format shell`).
