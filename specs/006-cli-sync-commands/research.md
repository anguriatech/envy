# Research: CLI Sync Commands (encrypt / decrypt)

**Feature**: 006-cli-sync-commands
**Date**: 2026-03-22
**Status**: Complete — all decisions resolved

---

## Decision 1: Passphrase input crate

**Decision**: `dialoguer = "0.11"` + `console = "0.15"` (console is a transitive dependency of dialoguer, available for free)

**Rationale**:
- `dialoguer` provides a single, consistent API for both the double-entry confirmation prompt needed by `encrypt` and the single prompt needed by `decrypt`.
- `Password::with_theme(&ColorfulTheme::default()).with_confirmation("Confirm passphrase", "Passphrases do not match.")` handles the full encrypt flow in one call — no custom retry logic required.
- The bundled `console` crate provides `console::style("text").green()` / `.yellow()` for coloured terminal output and automatic TTY detection (colour suppressed when stdout is not a terminal).
- TTY detection is built in: when stdin is not a TTY (CI/CD redirect), `Password::interact()` returns an error that the CLI can catch to enforce `ENVY_PASSPHRASE` fallback.

**Alternatives considered**:
- `rpassword` (v7.4.0): Too minimal — no confirmation prompt, no colour. Would require implementing both manually.
- `inquire` (v0.9.1): Functionally equivalent to dialoguer but heavier; no benefit for this feature's scope.

**Cargo.toml change**:
```toml
dialoguer = "0.11"
```
(`console` requires no explicit entry — comes transitively.)

---

## Decision 2: Coloured output approach

**Decision**: Use `console::style("text").green()` / `.yellow().dim()` inline in the command handler output.

**Rationale**:
- `console` is already in the dependency graph via `dialoguer` — zero additional dependency.
- `console` respects `NO_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` environment variables, and automatically disables ANSI codes when stdout is not a TTY (piped output). This satisfies Constitution Principle II (Determinism).
- Keeps colour logic in the CLI handler functions (the only layer allowed to produce terminal output).

**Alternatives considered**:
- Raw ANSI escape codes (`\x1b[32m`): No TTY detection, fails `NO_COLOR` convention.
- `termcolor` crate: Lower-level, cross-platform; would be the right choice if dialoguer weren't already pulling in `console`.

---

## Decision 3: CI/CD headless passphrase env var name

**Decision**: `ENVY_PASSPHRASE` is the canonical env var for the artifact passphrase in headless mode.

**Rationale**:
- `ENVY_MASTER_KEY` is reserved for the OS keyring vault master key (existing behaviour). Using the same name for the artifact passphrase would create dangerous ambiguity — two completely different secrets sharing a name.
- `ENVY_PASSPHRASE` is unambiguous and self-documenting: it is the GitOps artifact passphrase, not the per-machine vault key.
- This matches the spec (FR-006) and the Assumptions section of `spec.md`.

**Usage in code**:
```rust
std::env::var("ENVY_PASSPHRASE").ok().filter(|p| !p.trim().is_empty())
```

---

## Decision 4: `envy.enc` file path resolution

**Decision**: `envy.enc` is always written to and read from the directory that contains `envy.toml` (the project root), not the current working directory.

**Rationale**:
- `find_manifest` already returns the path to the `envy.toml` directory. Using its parent means `envy.enc` always sits in the canonical project root regardless of which subdirectory the user runs the command from. This is consistent with how `git` and `cargo` resolve project-root files.
- The existing CLI already calls `find_manifest(&cwd)` to obtain `(manifest, manifest_path)`. The artifact path is simply `manifest_path.parent().unwrap().join("envy.enc")`.

**Code pattern**:
```rust
let (manifest, manifest_path) = crate::core::find_manifest(&cwd)?;
let artifact_path = manifest_path.parent()
    // Safety: manifest_path always has a parent (it is not the root "/" — it is a file
    // returned by find_manifest which traverses up from cwd).
    .expect("manifest path must have a parent directory")
    .join("envy.enc");
```

---

## Decision 5: Passphrase memory safety

**Decision**: Wrap all passphrase strings in `Zeroizing<String>` immediately after reading from the terminal or environment.

**Rationale**:
- Constitution Principle I: in-memory representations of secrets MUST be zeroed as early as possible.
- `dialoguer::Password::interact()` returns a plain `String`. The CLI handler must immediately wrap the return value: `Zeroizing::new(passphrase_string)`.
- This `Zeroizing<String>` is then passed by reference (as `&str` via `.as_ref()`) to `seal_artifact` / `unseal_artifact` which already expect `&str`.

---

## Decision 6: Passphrase confirmation mismatch handling

**Decision**: A passphrase confirmation mismatch during `encrypt` exits with a non-zero code and a clear error message. No retry loop is offered in the first implementation.

**Rationale**:
- `dialoguer::Password::with_confirmation()` retries automatically if the two entries differ (this is its default behaviour). The CLI gets a single `Result` back: either the confirmed passphrase, or an IO error.
- If `interact()` returns an error in interactive mode (user cancelled via Ctrl-C, or confirmation failed after too many attempts), we map it to `CliError::PassphraseInput` and exit 2.

---

## Decision 7: New `CliError` variants for sync commands

**Decision**: Add two variants to `CliError`:
- `PassphraseInput(String)` — terminal read failed (IO error or Ctrl-C)
- `NothingImported` — zero environments decrypted; exit 1 with guidance message

**Rationale**:
- The existing `CliError` enum handles CLI-layer failures cleanly. Sync commands introduce two new CLI-layer failure modes not covered by `SyncError` (which is Core-layer).
- `NothingImported` is intentionally CLI-layer: the Core function `unseal_artifact` succeeds even when all environments are skipped (`SyncError::NothingImported` exists but is documented as "reserved for the CLI layer"). The CLI promotes it to a user-facing error only after checking `result.imported.is_empty()`.

---

## Decision 8: Module structure (no new files needed)

**Decision**: Add `cmd_encrypt` and `cmd_decrypt` to the existing `src/cli/commands.rs`. Add two new `Commands` variants to the existing `src/cli/mod.rs`. No new files required.

**Rationale**:
- The existing `commands.rs` pattern (`pub(super) fn cmd_*`) is the established convention. All seven current handlers live there; adding two more maintains structural consistency.
- No new Rust module files means no changes to `mod` declarations or public re-exports.

---

## Decision 9: Exit code for sync errors

**Decision**: Sync-specific exit codes follow the established exit-code table:

| Code | Condition |
|------|-----------|
| 0    | Success (all or partial import with at least one env imported) |
| 1    | `envy.enc` not found; zero environments imported (`NothingImported`) |
| 2    | Passphrase input error; empty passphrase |
| 4    | Vault error; malformed artifact; unsupported version |

**Rationale**: Consistent with the exit-code table already established in `contracts/cli.md` from feature 004.
