# Implementation Plan: Multi-Environment Encryption and Smart Merging

**Branch**: `009-multi-env-encrypt` | **Date**: 2026-03-25 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/009-multi-env-encrypt/spec.md`

## Summary

Overhaul `envy encrypt` to support independent per-environment passphrases, atomic writes with smart merging, key-rotation protection, interactive multi-environment selection, and a Diceware passphrase generator. The implementation adds one new crypto module (`diceware.rs`), two new core functions (`seal_env`, `write_artifact_atomic`, `check_envelope_passphrase`), and completely rewrites `cmd_encrypt` in the CLI layer.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.85)
**Primary Dependencies**: `rand = "0.8"` (new direct dep; already in `Cargo.lock` transitively), `dialoguer = "0.11"` (already present; `MultiSelect` + `Confirm` used), `serde_json` (already present)
**Storage**: SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (existing vault, read-only for this feature)
**Testing**: `cargo test` — unit tests in `diceware.rs`, refactored tests in `commands.rs`
**Target Platform**: Linux, macOS, Windows (same CI matrix)
**Project Type**: CLI tool
**Performance Goals**: Argon2id KDF is the bottleneck (~0.3–0.5 s per environment on standard hardware); acceptable for interactive use. Headless pipelines with 3 environments: < 2 seconds total.
**Constraints**: Constitution Principle I — passphrases must be `Zeroizing<String>` throughout. Principle IV — CLI must not call `crate::crypto` directly except for the two permitted exceptions.
**Scale/Scope**: Medium feature — 1 new crypto module, 3 new core functions, full rewrite of `cmd_encrypt`, 1 new data asset.

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | ✓ PASS | All passphrases wrapped in `Zeroizing<String>`; Diceware uses OS CSPRNG (`OsRng`); no plaintext writes; atomic write prevents partial files |
| II. Determinism | ✓ PASS | Diceware uses documented CSPRNG (not seeded RNG); output format unchanged for existing `table` cases; all error messages are stable strings |
| III. Rust Best Practices | ✓ PASS | All new code uses `Result<T, E>`, no `unwrap()` without justification; unit tests for `diceware.rs` and new core functions required |
| IV. Modularity (4-layer) | ✓ PASS | `diceware.rs` in Crypto layer; `seal_env`/`write_artifact_atomic`/`check_envelope_passphrase` in Core layer; all interactive UI and passphrase resolution stays in CLI layer |
| V. Language | ✓ PASS | All identifiers, docs, and messages in English |

## Project Structure

### Documentation (this feature)

```text
specs/009-multi-env-encrypt/
├── plan.md              ← this file
├── research.md          ← Phase 0 output
├── contracts/
│   └── encrypt-command.md
├── quickstart.md
└── tasks.md             ← /speckit.tasks output (not created here)
```

### Source Code Changes

```text
data/
  eff-wordlist.txt        — NEW: EFF Large Wordlist (7776 words, ~90 KiB), embedded at compile time

src/
  crypto/
    mod.rs                — add `pub mod diceware;` and re-export `suggest_passphrase`
    diceware.rs           — NEW: suggest_passphrase() → String using OsRng + EFF wordlist

  core/
    sync.rs               — add seal_env(), write_artifact_atomic(), check_envelope_passphrase()
                            refactor write_artifact to delegate to write_artifact_atomic
    mod.rs                — re-export new sync functions

  cli/
    commands.rs           — rewrite cmd_encrypt(); add resolve_passphrase_for_env();
                            add print_rotation_warning(), print_diceware_banner()

Cargo.toml                — add `rand = "0.8"` to [dependencies]
```

---

## Milestone 1 — Diceware Engine (`src/crypto/diceware.rs`)

**Goal**: A standalone module that generates a cryptographically random Diceware passphrase. No CLI wiring — fully unit-testable in isolation.

### 1.1 Add data asset

Create `data/eff-wordlist.txt` in the repo root. The EFF Large Wordlist format is:

```
11111	abacus
11112	abdomen
...
```

Each line: 5-digit dice roll (ignored), tab, word. The file contains 7776 lines.

### 1.2 New module `src/crypto/diceware.rs`

```rust
//! Diceware passphrase generator using the EFF Large Wordlist.
//!
//! Uses the OS CSPRNG exclusively (Constitution Principle I).
//! The word list is embedded at compile time; no file I/O at runtime.

use rand::rngs::OsRng;
use rand::seq::SliceRandom as _;

/// Raw EFF Large Wordlist embedded at compile time.
const WORDLIST_RAW: &str = include_str!("../../data/eff-wordlist.txt");

/// Returns a slice of all 7776 words parsed from WORDLIST_RAW.
///
/// Parsing is done once and the result is cached via `std::sync::OnceLock`.
fn words() -> &'static [&'static str] {
    use std::sync::OnceLock;
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        WORDLIST_RAW
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .collect()
    })
}

/// Generates a Diceware passphrase of `word_count` words separated by spaces.
///
/// Uses [`OsRng`] (OS CSPRNG) — never a seeded or deterministic RNG.
///
/// # Panics
/// Panics if the word list is empty (structurally impossible with the embedded asset).
pub fn suggest_passphrase(word_count: usize) -> String {
    let w = words();
    let mut rng = OsRng;
    let chosen: Vec<&str> = (0..word_count)
        .map(|_| *w.choose(&mut rng).expect("word list must not be empty"))
        .collect();
    chosen.join(" ")
}
```

### 1.3 Export from crypto layer

In `src/crypto/mod.rs`:
```rust
pub mod diceware;
pub use diceware::suggest_passphrase;
```

### 1.4 Unit tests (inside `src/crypto/diceware.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn wordlist_has_7776_entries() { assert_eq!(words().len(), 7776); }
    #[test] fn suggest_4_words_has_3_spaces() { assert_eq!(suggest_passphrase(4).matches(' ').count(), 3); }
    #[test] fn suggest_is_non_empty() { assert!(!suggest_passphrase(4).is_empty()); }
    #[test] fn two_suggestions_differ() {
        // Probabilistic: P(collision) = 1/7776^4 ≈ 2.8e-16
        assert_ne!(suggest_passphrase(4), suggest_passphrase(4));
    }
}
```

---

## Milestone 2 — Core/Sync Modifications (`src/core/sync.rs`)

**Goal**: Three new functions that give the CLI layer the primitives it needs for smart merge, atomic write, and pre-flight check. Existing `seal_artifact` and `write_artifact` remain unchanged (backward compatibility for existing tests).

### 2.1 `seal_env` — seal a single environment

```rust
/// Reads all secrets for `env_name` from the vault and seals them into
/// one [`EncryptedEnvelope`] using `passphrase`.
///
/// Used by `cmd_encrypt` to build the merge map one environment at a time,
/// each potentially with a different passphrase.
///
/// # Errors
/// - [`SyncError::Artifact(ArtifactError::WeakPassphrase)`] if passphrase is empty.
/// - [`SyncError::VaultError`] if reading secrets from the vault fails.
pub fn seal_env(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
    passphrase: &str,
) -> Result<EncryptedEnvelope, SyncError> {
    if passphrase.trim().is_empty() {
        return Err(SyncError::Artifact(ArtifactError::WeakPassphrase));
    }
    let secrets_map = crate::core::get_env_secrets(vault, master_key, project_id, env_name)
        .map_err(|e| SyncError::VaultError(e.to_string()))?;
    let secrets: BTreeMap<String, Zeroizing<String>> = secrets_map.into_iter().collect();
    let payload = ArtifactPayload { secrets };
    Ok(seal_envelope(passphrase, &payload)?)
}
```

### 2.2 `write_artifact_atomic` — atomic write via tmp + rename

```rust
/// Serializes `artifact` to pretty-printed JSON and writes it atomically.
///
/// Writes to `envy.enc.tmp` (sibling of `path`) then calls `std::fs::rename`.
/// A crash between the write and the rename leaves the previous file intact.
/// Both paths must be on the same filesystem (always true: same directory).
///
/// # Errors
/// - [`SyncError::Io`] on serialization, write, or rename failure.
pub fn write_artifact_atomic(artifact: &SyncArtifact, path: &Path) -> Result<(), SyncError> {
    let tmp = path.with_file_name("envy.enc.tmp");
    let json = serde_json::to_string_pretty(artifact)
        .map_err(|e| SyncError::Io(e.to_string()))?;
    std::fs::write(&tmp, json.as_bytes())
        .map_err(|e| SyncError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| SyncError::Io(format!("atomic rename failed: {e}")))?;
    Ok(())
}
```

Also update `write_artifact` to delegate to `write_artifact_atomic`:
```rust
pub fn write_artifact(artifact: &SyncArtifact, path: &Path) -> Result<(), SyncError> {
    write_artifact_atomic(artifact, path)
}
```

### 2.3 `check_envelope_passphrase` — pre-flight decryption check

```rust
/// Returns `true` if `passphrase` successfully decrypts `envelope`.
///
/// Used by `cmd_encrypt` to detect key rotation (FR-008): if `false`, the
/// user is prompted to confirm before overwriting with the new passphrase.
///
/// Authentication failures (wrong passphrase OR tampered ciphertext) both
/// return `false` — AES-GCM cannot distinguish the two cases.
pub fn check_envelope_passphrase(
    passphrase: &str,
    env_name: &str,
    envelope: &EncryptedEnvelope,
) -> bool {
    unseal_envelope(passphrase, env_name, envelope).is_ok()
}
```

### 2.4 Re-export from `src/core/mod.rs`

```rust
pub use sync::{
    // existing
    seal_artifact, unseal_artifact, read_artifact, write_artifact,
    SyncError, UnsealResult,
    // new
    seal_env, write_artifact_atomic, check_envelope_passphrase,
};
```

---

## Milestone 3 — CLI Orchestration (`src/cli/commands.rs`)

**Goal**: Rewrite `cmd_encrypt` to support per-env passphrases, interactive selection, smart merge, atomic writes, pre-flight check, and Diceware suggestion. Add `resolve_passphrase_for_env`.

### 3.1 `resolve_passphrase_for_env`

```rust
/// Resolves the passphrase for a single named environment.
///
/// Priority order:
/// 1. `ENVY_PASSPHRASE_<UPPER_NORMALISED>` — e.g., `my-env` → `ENVY_PASSPHRASE_MY_ENV`
/// 2. `ENVY_PASSPHRASE` — global fallback
/// 3. Interactive terminal prompt (with optional Diceware suggestion)
///
/// Returns `Ok(None)` in headless mode when NEITHER env var resolves AND no
/// TTY is available — used by `cmd_encrypt` to skip an environment silently.
fn resolve_passphrase_for_env(
    env_name: &str,
    confirm: bool,
    suggested: Option<&str>,
) -> Result<Option<zeroize::Zeroizing<String>>, CliError>
```

**Normalisation**: `env_name.to_uppercase().replace('-', "_")`

**Whitespace guard**: if either env var is set but whitespace-only → immediate `Err(CliError::PassphraseInput(...))` (same pattern as existing `resolve_passphrase`).

**Interactive with suggestion**: When `suggested.is_some()`, display the suggestion as part of the prompt:
```
Enter passphrase for 'development'
  Suggested: "correct-horse-battery-staple"
  [press Enter to accept, or type your own]
Passphrase: _
```
If the user presses Enter and the prompt returns an empty string, use the suggestion.

### 3.2 Headless mode detection

```rust
/// Returns true if at least one ENVY_PASSPHRASE* env var is set (non-whitespace).
/// Used to decide whether to show the interactive MultiSelect menu.
fn is_headless_mode(env_names: &[String]) -> bool {
    if std::env::var("ENVY_PASSPHRASE")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    env_names.iter().any(|name| {
        let key = format!(
            "ENVY_PASSPHRASE_{}",
            name.to_uppercase().replace('-', "_")
        );
        std::env::var(&key)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    })
}
```

### 3.3 Rewritten `cmd_encrypt` algorithm

```
1. List all environment names from vault → env_names: Vec<String>
   If empty → print "No environments found." → return Ok(())

2. Determine headless vs interactive mode (is_headless_mode(&env_names))

3. Determine selected_envs: Vec<String>
   - Headless: all env_names (each will be resolved individually; those without
     a resolvable passphrase will be skipped)
   - Interactive + env_filter set: [env_filter.to_owned()]
   - Interactive + no filter: MultiSelect(env_names) → selected subset

4. If selected_envs is empty → print "Nothing to encrypt." → return Ok(())

5. Read existing artifact:
   - read_artifact(artifact_path):
     - Ok(a)   → existing_artifact = a
     - Err(FileNotFound) → existing_artifact = empty SyncArtifact
     - Err(other)        → abort with error (malformed JSON, FR-013)

6. For each env_name in selected_envs:
   a. Determine if new (not in existing_artifact.environments)

   b. If headless: resolve_passphrase_for_env(env_name, false, None)
      - Returns Ok(None) → skip this env silently
      - Returns Ok(Some(pass)) → proceed
      - Returns Err → propagate

   c. If interactive:
      - If new env: generate diceware suggestion = suggest_passphrase(4)
        Resolve with confirm=true, suggested=Some(&suggestion)
        If user accepted suggestion → print_diceware_banner(&suggestion)
      - If existing env: resolve with confirm=false, suggested=None

   d. Pre-flight check (interactive, existing env only):
      - check_envelope_passphrase(&passphrase, env_name, &existing_envelope)
      - If false: show rotation warning
        - Confirm::new("Continue and rotate key?").default(false).interact()
        - If No → skip this env (continue loop)
        - If Yes → proceed

   e. seal_env(vault, master_key, project_id, env_name, &passphrase) → envelope
   f. existing_artifact.environments.insert(env_name, envelope)

7. write_artifact_atomic(&existing_artifact, artifact_path)

8. Print success:
   "Sealed N environment(s) → <path>"
   "  ✓  <env_name>" for each updated env
```

### 3.4 `print_diceware_banner`

```rust
fn print_diceware_banner(passphrase: &str) {
    use dialoguer::console::style;
    eprintln!();
    eprintln!("{}", style("╔══════════════════════════════════════╗").yellow().bold());
    eprintln!("{}", style("║         ⚠  SAVE THIS PASSPHRASE  ⚠  ║").yellow().bold());
    eprintln!("{}", style("╠══════════════════════════════════════╣").yellow().bold());
    eprintln!("  {}", style(passphrase).cyan().bold());
    eprintln!("{}", style("╚══════════════════════════════════════╝").yellow().bold());
    eprintln!("  You will not be shown this passphrase again.");
    eprintln!();
}
```

### 3.5 Rotation warning helper

```rust
fn confirm_key_rotation(env_name: &str) -> Result<bool, CliError> {
    eprintln!(
        "\n⚠  Passphrase does not match existing data for '{env_name}'.\n   \
         Continuing will ROTATE the key. Anyone using the old passphrase\n   \
         will no longer be able to decrypt this environment."
    );
    dialoguer::Confirm::new()
        .with_prompt("Continue and rotate key?")
        .default(false)
        .interact()
        .map_err(|e| CliError::PassphraseInput(e.to_string()))
}
```

---

## Complexity Tracking

No constitution violations. All new code stays within its designated layer:
- `diceware.rs` → Crypto layer (no imports from CLI/Core/DB)
- `seal_env`, `write_artifact_atomic`, `check_envelope_passphrase` → Core layer
- `resolve_passphrase_for_env`, `cmd_encrypt` rewrite → CLI layer
