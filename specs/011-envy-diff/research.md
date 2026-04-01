# Research: Pre-Encrypt Secret Diff

**Feature**: `011-envy-diff`
**Date**: 2026-03-28

---

## R1: Can we reuse `unseal_envelope` / `unseal_env` without modification?

**Decision**: Yes — full reuse, zero modifications required.

**Rationale**: `unseal_env` (in `src/core/sync.rs`) already provides exactly the interface we need:

```rust
pub fn unseal_env(
    artifact: &SyncArtifact,
    env_name: &str,
    passphrase: &str,
) -> Result<Option<BTreeMap<String, Zeroizing<String>>>, SyncError>
```

- Returns `Ok(Some(secrets))` on success — gives us the artifact-side `BTreeMap` directly.
- Returns `Ok(None)` on wrong passphrase — we can map this to a CLI-level auth error.
- Returns `Err(SyncError)` only for structural issues — maps cleanly to existing error handling.

The only subtlety: `unseal_env` returns `Ok(None)` for *both* "environment not in artifact" and "wrong passphrase" (AES-GCM auth tag failure). For `envy diff`, we need to distinguish these cases. Solution: check `artifact.environments.contains_key(env_name)` *before* calling `unseal_env`. If the key is absent, skip passphrase resolution entirely. If present and `unseal_env` returns `None`, it's an auth failure.

**Alternatives considered**: Creating a new `unseal_env_strict` that returns a distinguishable error for auth failures. Rejected — the check-then-unseal approach requires no changes to the crypto/sync layer.

---

## R2: Can we reuse `get_env_secrets` for the vault side?

**Decision**: Yes — direct reuse.

**Rationale**: `get_env_secrets` (in `src/core/ops.rs`) returns `HashMap<String, Zeroizing<String>>` — all decrypted secrets for an environment. For the diff, we convert this to a `BTreeMap` for consistent sorted iteration matching the artifact side's `BTreeMap`.

```rust
pub fn get_env_secrets(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
) -> Result<HashMap<String, Zeroizing<String>>, CoreError>
```

Note: this returns an *empty* `HashMap` (not an error) when the environment doesn't exist in the vault. This is the correct behavior for `envy diff` — an empty vault side means all artifact secrets are deletions.

---

## R3: Colored output — new dependency or inline ANSI?

**Decision**: Inline ANSI escape codes with TTY + `NO_COLOR` detection. No new crate.

**Rationale**: The diff output is plain text with color emphasis, not a table. `comfy_table` handles colors for table cells but isn't suitable for free-form text. Adding `console` (transitive via `dialoguer`) as a direct dependency would work, but the color needs are minimal:

| Symbol | Color  | ANSI code |
|--------|--------|-----------|
| `+`    | Green  | `\x1b[32m` |
| `-`    | Red    | `\x1b[31m` |
| `~`    | Yellow | `\x1b[33m` |

The detection logic is two lines:

```rust
fn is_color_enabled() -> bool {
    use std::io::IsTerminal;
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
}
```

`std::io::IsTerminal` is stable since Rust 1.70 (we target MSRV 1.85). This follows the `NO_COLOR` convention (https://no-color.org/) and matches `comfy_table`'s own detection behavior.

**Alternatives considered**:
1. **`console` crate as direct dep** — Adds explicit dependency for 3 colors. Overkill.
2. **`comfy_table` single-column table** — Abuses the table abstraction. The diff output is a list, not tabular data.
3. **`colored` crate** — Another new dep for minimal usage. Rejected.

---

## R4: Exit code 1 for "differences found" — return type design

**Decision**: `cmd_diff` returns `Result<bool, CliError>` where `true` = differences found.

**Rationale**: The `diff(1)` convention requires exit code 1 for "differences found" — this is a *successful* outcome, not an error. All existing `cmd_*` handlers return `Result<(), CliError>`, but `envy diff` needs to signal three states: clean (0), drift (1), error (2+).

Returning `bool` keeps the dispatch in `run()` explicit:

```rust
Commands::Diff { env, reveal } => {
    match commands::cmd_diff(&vault, &master_key, &project_id, env, &artifact, cli.format, reveal) {
        Ok(has_diff) => if has_diff { 1 } else { 0 },
        Err(e) => { eprintln!(...); cli_exit_code(&e) }
    }
}
```

**Alternatives considered**:
1. **Return `Result<(), CliError>` with a `CliError::DiffFound` variant** — Semantically wrong; "differences found" is not an error.
2. **Return `Result<i32, CliError>`** — Leaks exit-code concerns into the command handler.
3. **Custom enum `DiffOutcome { Clean, Drift }`** — Functionally identical to `bool` but heavier.

---

## R5: JSON output — `old_value`/`new_value` key presence

**Decision**: Use `serde_json::Value` construction to conditionally include keys.

**Rationale**: The contract specifies that without `--reveal`, the `old_value` and `new_value` keys must be *entirely absent* from the JSON — not `null`, not `"***"`, but missing. This prevents accidental exposure through JSON key enumeration or schema-aware tooling.

Serde's `#[serde(skip_serializing_if)]` could achieve this, but requires wrapping values in `Option<Option<String>>` (outer option = key presence, inner = null vs value). Building the JSON programmatically with `serde_json::json!()` is cleaner:

```rust
let mut entry = serde_json::json!({ "key": e.key, "type": type_str });
if reveal {
    entry["old_value"] = /* ... */;
    entry["new_value"] = /* ... */;
}
```

**Alternatives considered**:
1. **Two separate serde structs** (`DiffChangeJson` vs `DiffChangeRevealJson`) — Code duplication, hard to maintain.
2. **`#[serde(skip_serializing_if)]` with nested Options** — Complex type, confusing semantics.

---

## R6: New CliError variants needed

**Decision**: Add two new variants to `CliError` for diff-specific error conditions.

**Rationale**: The contract specifies distinct exit codes for diff errors:

| Exit | Condition | Existing variant? | Decision |
|------|-----------|-------------------|----------|
| 2 | Wrong passphrase | `PassphraseInput` (exit 2) | Reuse — message is different but exit code matches |
| 3 | Env not found in vault or artifact | None (exit 3 = init conflict only) | **New: `EnvNotFound(String)`** |
| 4 | Vault failure | `VaultOpen` (exit 4) | Reuse |
| 5 | Malformed artifact | None | **New: `ArtifactUnreadable(String)`** |

Two new variants is minimal. Reusing `PassphraseInput` for "wrong passphrase" works because the exit code (2) matches and the error message distinguishes the cases.

---

## R7: Dependency audit — no new crates

**Decision**: Zero new crates required.

| Need | Satisfied by |
|------|-------------|
| Vault secret retrieval | `core::get_env_secrets` (existing) |
| Artifact unsealing | `core::unseal_env` (existing) |
| Artifact reading | `core::read_artifact` (existing) |
| Passphrase resolution | `cli::resolve_passphrase_for_env` (existing) |
| JSON output | `serde_json` (existing) |
| Terminal colors | ANSI codes + `std::io::IsTerminal` (stdlib) |
| Memory zeroing | `zeroize` (existing) |
| Sorted iteration | `std::collections::BTreeMap` (stdlib) |

The entire feature is built from existing building blocks plus one new pure function (`compute_diff`).
