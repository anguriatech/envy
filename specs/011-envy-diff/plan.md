# Implementation Plan: Pre-Encrypt Secret Diff

**Branch**: `011-envy-diff` | **Date**: 2026-03-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/011-envy-diff/spec.md`

## Summary

Add `envy diff [-e ENV] [--reveal] [--format table|json]` — a read-only command that compares the local vault's secrets against the sealed `envy.enc` artifact for a single environment, producing a Git-style diff of additions, deletions, and modifications. By default only key names are shown; `--reveal` exposes values. Exit code 0 = clean, 1 = drift, 2+ = error. No new crates. No schema changes.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.85)
**Primary Dependencies**: `clap` (derive), `serde_json`, `zeroize`, `comfy-table` (unused for diff — ANSI codes instead)
**Storage**: SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (read-only for this feature)
**Testing**: `cargo test` (unit + integration), E2E bash script
**Target Platform**: Linux, macOS, Windows (3-OS CI matrix)
**Project Type**: CLI tool
**Performance Goals**: <5s for 500-secret environment diff (SC-001)
**Constraints**: Zero secret values in output by default (FR-003, SC-003); memory-zeroed containers (FR-014)
**Scale/Scope**: Single environment per invocation; up to 1,000 keys

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | PASS | Values never printed without `--reveal`. All `Zeroizing<String>` containers. No disk writes. |
| II. Determinism | PASS | Same inputs → same diff output. Exit codes are stable (0/1/2+). Sorted alphabetically by key. |
| III. Rust Best Practices | PASS | Typed errors (`CliError` variants). No `.unwrap()` without justification. Unit tests for core logic. `cargo clippy -D warnings`. |
| IV. Modularity (4-layer) | PASS | Core layer: pure `compute_diff`. CLI layer: orchestration + rendering. Crypto layer: untouched. DB layer: untouched. |
| V. Language | PASS | All English. |

No violations. Complexity Tracking table not needed.

---

## Architecture

### Layer Responsibilities

```
┌─────────────────────────────────────────────────────────────┐
│  CLI Layer (src/cli/)                                       │
│                                                             │
│  cmd_diff():                                                │
│    1. Fetch vault secrets  →  core::get_env_secrets()       │
│    2. Read artifact        →  core::read_artifact()         │
│    3. Check env in artifact (key lookup, no decryption)     │
│    4. Resolve passphrase   →  resolve_passphrase_for_env()  │
│    5. Unseal envelope      →  core::unseal_env()            │
│    6. Compute diff         →  core::compute_diff()          │
│    7. Render (table/JSON, reveal gate)                      │
│                                                             │
│  New: Diff variant in Commands enum                         │
│  New: cmd_diff handler                                      │
│  New: render_diff_table(), write_diff_json()                │
│  New: is_color_enabled(), colorize()                        │
│  New: CliError::EnvNotFound, CliError::ArtifactUnreadable   │
├─────────────────────────────────────────────────────────────┤
│  Core Layer (src/core/)                                     │
│                                                             │
│  NEW: src/core/diff.rs                                      │
│    - ChangeType enum { Added, Removed, Modified }           │
│    - DiffEntry struct                                       │
│    - DiffReport struct                                      │
│    - compute_diff() — pure function, no I/O                 │
│                                                             │
│  REUSE (unchanged):                                         │
│    - get_env_secrets()  — vault-side secrets                 │
│    - read_artifact()    — parse envy.enc                     │
│    - unseal_env()       — decrypt single envelope            │
├─────────────────────────────────────────────────────────────┤
│  Crypto Layer (src/crypto/)         — NO CHANGES            │
├─────────────────────────────────────────────────────────────┤
│  DB Layer (src/db/)                 — NO CHANGES            │
│                                     — NO SCHEMA MIGRATION   │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

```
Vault (SQLite)                    Artifact (envy.enc)
      │                                 │
      ▼                                 ▼
get_env_secrets()              read_artifact()
      │                                 │
      ▼                                 ▼
HashMap<Key, Zeroizing>    SyncArtifact{environments}
      │                                 │
      │                     ┌───────────┤
      │                     ▼           │
      │            env in artifact?     │
      │               │       │         │
      │             yes      no         │
      │               │       │         │
      │               ▼       │         │
      │        resolve_passphrase       │
      │               │       │         │
      │               ▼       │         │
      │          unseal_env() │         │
      │               │       │         │
      │               ▼       ▼         │
      │         BTreeMap   empty        │
      │               │       │         │
      ▼               ▼       ▼         │
  BTreeMap ──────► compute_diff() ◄─────┘
                       │
                       ▼
                   DiffReport
                       │
              ┌────────┴────────┐
              ▼                 ▼
        render_table()    write_json()
         (stdout)          (stdout)
              │                 │
              ▼                 ▼
         exit 0 or 1       exit 0 or 1
```

---

## New File: `src/core/diff.rs`

### Types

```rust
/// The kind of change detected for a single secret key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Key exists in vault but not in artifact.
    Added,
    /// Key exists in artifact but not in vault.
    Removed,
    /// Key exists in both but values differ.
    Modified,
}

/// A single key-level difference between vault and artifact.
#[derive(Debug)]
pub struct DiffEntry {
    pub key: String,
    pub change: ChangeType,
    /// Value from the artifact (None for Added entries).
    pub old_value: Option<Zeroizing<String>>,
    /// Value from the vault (None for Removed entries).
    pub new_value: Option<Zeroizing<String>>,
}

/// Complete diff result for one environment.
#[derive(Debug)]
pub struct DiffReport {
    pub env_name: String,
    pub entries: Vec<DiffEntry>,  // sorted alphabetically by key
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl DiffReport {
    /// Returns true if there are any differences.
    pub fn has_differences(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Total number of changes.
    pub fn total(&self) -> usize {
        self.added + self.removed + self.modified
    }
}
```

### Function

```rust
/// Compare vault secrets against artifact secrets for a single environment.
///
/// Both inputs are consumed (moved) to ensure Zeroizing values are properly
/// dropped after comparison. The returned DiffReport retains values only for
/// the entries that differ — unchanged keys are dropped immediately.
///
/// # Arguments
/// - `env_name`: environment name (for the report header).
/// - `vault_secrets`: decrypted secrets from the local vault.
/// - `artifact_secrets`: decrypted secrets from the artifact envelope,
///   or an empty BTreeMap if the artifact is missing / env not in artifact.
///
/// # Returns
/// A DiffReport with entries sorted alphabetically by key.
pub fn compute_diff(
    env_name: &str,
    vault_secrets: BTreeMap<String, Zeroizing<String>>,
    artifact_secrets: BTreeMap<String, Zeroizing<String>>,
) -> DiffReport
```

**Algorithm**: Iterate the union of keys from both maps (BTreeMap gives sorted order). For each key:
- Present in vault only → `Added`
- Present in artifact only → `Removed`
- Present in both, values differ (byte comparison) → `Modified`
- Present in both, values equal → skip (no entry)

The algorithm runs in O(n) where n = |vault_keys ∪ artifact_keys| since both BTreeMaps are sorted.

---

## Modified File: `src/core/mod.rs`

Add the new module and re-export:

```rust
pub mod diff;

pub use diff::{ChangeType, DiffEntry, DiffReport, compute_diff};
```

---

## Modified File: `src/cli/mod.rs`

### New Command Variant

```rust
/// Compare local vault secrets against the sealed envy.enc artifact.
///
/// Shows additions, deletions, and modifications for one environment.
/// By default only key names are shown — use `--reveal` to include values.
/// Exit code 0 = no differences, 1 = differences found, 2+ = error.
#[command(visible_alias = "df")]
Diff {
    /// Target environment (default: development).
    #[arg(short = 'e', long = "env", value_name = "ENV", default_value = "development")]
    env: String,

    /// Show decrypted secret values in the output.
    #[arg(long)]
    reveal: bool,
},
```

### Dispatch in `run()`

```rust
Commands::Diff { env, reveal } => {
    let artifact = artifact_path(&manifest_path);
    match commands::cmd_diff(
        &vault, &master_key, &project_id, &env, &artifact, cli.format, reveal,
    ) {
        Ok(has_diff) => if has_diff { 1 } else { 0 },
        Err(e) => {
            eprintln!("{}", format_cli_error(&e));
            cli_exit_code(&e)
        }
    }
}
```

Note: `cmd_diff` returns `Result<bool, CliError>` — the only `cmd_*` handler that returns `bool` (see [research.md R4](research.md)).

---

## Modified File: `src/cli/error.rs`

### New Variants

```rust
/// Target environment not found in vault or artifact.
#[error("environment '{0}' not found in vault or artifact")]
EnvNotFound(String),

/// Artifact exists but is malformed or uses an unsupported version.
#[error("envy.enc is unreadable: {0}")]
ArtifactUnreadable(String),
```

### Updated Exit Code Map

```rust
CliError::EnvNotFound(_) => 3,
CliError::ArtifactUnreadable(_) => 5,
```

---

## Modified File: `src/cli/commands.rs`

### `cmd_diff` Handler

```rust
pub(super) fn cmd_diff(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
    artifact_path: &Path,
    format: OutputFormat,
    reveal: bool,
) -> Result<bool, CliError>
```

**Orchestration flow**:

1. **Vault side**: Call `core::get_env_secrets(vault, master_key, project_id, env_name)`. Convert `HashMap` → `BTreeMap`.

2. **Artifact side**: Call `core::read_artifact(artifact_path)`.
   - `Err(SyncError::FileNotFound(_))` → artifact missing, use empty `BTreeMap`, set `artifact_missing = true`.
   - `Err(other)` → return `CliError::ArtifactUnreadable(msg)`.
   - `Ok(artifact)` → check if `artifact.environments.contains_key(env_name)`.

3. **Passphrase & unseal** (only if artifact exists AND env is in it):
   - Call `resolve_passphrase_for_env(env_name, false, None)`.
   - If `Ok(None)` (no TTY, no env var) → return `CliError::PassphraseInput(msg)`.
   - Call `core::unseal_env(&artifact, env_name, &passphrase)`.
   - If `Ok(None)` → wrong passphrase → return `CliError::PassphraseInput("incorrect passphrase for environment '...'")`.
   - If `Ok(Some(secrets))` → use as artifact side.

4. **Both sides empty?**: If vault has no env AND artifact has no env → return `CliError::EnvNotFound(env_name)`.

5. **Compute diff**: Call `core::compute_diff(env_name, vault_btree, artifact_btree)`.

6. **Render**:
   - If `reveal` → `eprintln!("⚠ Warning: secret values are visible in the output below.");`
   - If `format == Json` → call `write_diff_json(&report, reveal, &mut stdout())`.
   - Else → call `render_diff_table(&report, reveal, artifact_missing)`.

7. **Return**: `Ok(report.has_differences())`.

### Color Helper (private)

```rust
fn is_color_enabled() -> bool {
    use std::io::IsTerminal;
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
}

fn colorize(text: &str, ansi: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[{ansi}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}
// Green = "32", Red = "31", Yellow = "33"
```

### Table Renderer (private)

```rust
fn render_diff_table(
    report: &DiffReport,
    reveal: bool,
    artifact_missing: bool,
) -> ()
```

Outputs to stdout using `println!`. Follows the exact format from [contracts/diff-command.md](contracts/diff-command.md):
- Header line: `envy diff: {env} (vault ↔ envy.enc)`
- Optional notice for missing artifact / missing env
- Each entry: `  + KEY` / `  - KEY` / `  ~ KEY` (colored)
- With `--reveal`: indented `vault:` / `artifact:` value lines below each entry
- Summary line: `N changes: X added, Y removed, Z modified`
- "No differences" message when report is empty

### JSON Writer (private)

```rust
fn write_diff_json(
    report: &DiffReport,
    reveal: bool,
    writer: &mut impl Write,
) -> Result<(), CliError>
```

Builds `serde_json::Value` programmatically (see [research.md R5](research.md)). Conditionally includes `old_value`/`new_value` only when `reveal = true`. Takes `impl Write` for testability (tests pass `Vec<u8>`, production passes `stdout()`).

---

## Testing Strategy

### Core Layer Unit Tests (`src/core/diff.rs`)

| Test | Input | Assertion |
|------|-------|-----------|
| `diff_all_added` | vault: {A,B}, artifact: {} | 2 Added entries, sorted |
| `diff_all_removed` | vault: {}, artifact: {X,Y} | 2 Removed entries, sorted |
| `diff_all_modified` | vault: {A=new}, artifact: {A=old} | 1 Modified entry |
| `diff_mixed_changes` | vault: {A=same,B=new,D=added}, artifact: {A=same,B=old,C=removed} | B=Modified, C=Removed, D=Added; A excluded |
| `diff_no_changes` | vault: {A=v}, artifact: {A=v} | empty entries, has_differences = false |
| `diff_empty_both` | vault: {}, artifact: {} | empty, has_differences = false |
| `diff_sorted_output` | vault: {Z,A,M}, artifact: {} | entries ordered A, M, Z |
| `diff_values_retained` | vault: {K=new}, artifact: {K=old} | old_value = "old", new_value = "new" |

### CLI Layer Unit Tests (`src/cli/commands.rs`)

| Test | Approach | Assertion |
|------|----------|-----------|
| `diff_json_no_reveal` | `write_diff_json` to `Vec<u8>`, reveal=false | Valid JSON, no `old_value`/`new_value` keys in any entry |
| `diff_json_with_reveal` | `write_diff_json` to `Vec<u8>`, reveal=true | Valid JSON, `old_value`/`new_value` present in each entry |
| `diff_json_no_differences` | Empty DiffReport | `has_differences: false`, empty changes array |
| `diff_json_type_strings` | Mixed DiffReport | type values are `"added"`, `"removed"`, `"modified"` |

### E2E Scenario (bash script)

New scenario in `tests/e2e_devops_scenarios.sh`:
1. Init project, set 3 secrets (A, B, C), encrypt
2. Add D, modify B, remove C
3. Run `envy diff` → assert exit code 1
4. Run `envy diff --format json` → parse with `jq`, assert 3 changes (1 added, 1 removed, 1 modified)
5. Run `envy diff` with identical vault/artifact → assert exit code 0

---

## Project Structure

### Documentation (this feature)

```text
specs/011-envy-diff/
├── plan.md              # This file
├── research.md          # R1–R7 decisions
├── data-model.md        # DiffEntry, DiffReport entities
├── contracts/
│   └── diff-command.md  # CLI contract (signatures, output, exit codes)
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Task breakdown (generated by /speckit.tasks)
```

### Source Code (new and modified files)

```text
src/
├── core/
│   ├── mod.rs           # MODIFIED — add `pub mod diff;` + re-exports
│   └── diff.rs          # NEW — ChangeType, DiffEntry, DiffReport, compute_diff()
├── cli/
│   ├── mod.rs           # MODIFIED — add Diff variant + dispatch
│   ├── commands.rs      # MODIFIED — add cmd_diff, render_diff_table, write_diff_json, colorize
│   └── error.rs         # MODIFIED — add EnvNotFound, ArtifactUnreadable variants + exit codes
tests/
└── e2e_devops_scenarios.sh  # MODIFIED — add Scenario 9 (diff round-trip)
```

### Unchanged files

```text
src/crypto/*             # No modifications
src/db/*                 # No modifications, no schema migration
Cargo.toml               # No new dependencies
```

---

## Complexity Tracking

No constitution violations. Table not needed.
