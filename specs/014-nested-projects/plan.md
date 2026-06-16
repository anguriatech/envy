# Implementation Plan: Allow Nested Envy Projects

**Branch**: `014-nested-projects` | **Date**: 2026-06-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/014-nested-projects/spec.md`

## Summary

Allow `envy init` in subdirectories that have a parent envy project. The change is two small edits: one match arm in `cmd_init` that previously returned `ParentProjectExists` is changed to a no-op fall-through, and the `ParentProjectExists` variant is removed from `CliError`. `find_manifest`, `create_manifest`, and all other commands already support nested projects correctly.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.85)
**Primary Dependencies**: `clap` (derive), `dialoguer`, `serde_json`, `zeroize`, `thiserror`, `toml` — all already in `Cargo.toml`. No new crate added.
**Storage**: SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (existing vault, shared across all projects; each project has its own UUID).
**Testing**: `cargo test` for unit and integration tests. Two new unit tests in `src/cli/commands.rs`. The `find_manifest_in_parent_dir` test in `src/core/manifest.rs` continues to pass.
**Target Platform**: Linux + macOS + Windows (no platform-specific change).
**Project Type**: CLI tool (single binary).
**Constraints**: No new error variants; no new CLI flags; no new crate dependencies; no changes to `find_manifest` or `create_manifest`.
**Scale/Scope**: Two lines of production code changed; one error variant removed; two new unit tests; one doc paragraph.

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Security by Default** | ✓ Pass | No secrets are exposed. The vault continues to separate projects by UUID. |
| **II. Determinism** | ✓ Pass | The behaviour is deterministic: init in a subdirectory always succeeds (unless the cwd has envy.toml). |
| **III. Rust Best Practices** | ✓ Pass | No new `unwrap()` or `expect()`. Two new unit tests (zero prior coverage). |
| **IV. Modularity (4-layer)** | ✓ Pass | The change is in the CLI layer only. `find_manifest` (core) is unchanged. |
| **V. Language** | ✓ Pass | English only. |

## Project Structure

### Documentation (this feature)

```text
specs/014-nested-projects/
├── plan.md
├── spec.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── cli-init.md
└── checklists/
    └── requirements.md
```

### Source Code Changes

```
src/
├── cli/
│   ├── commands.rs   → cmd_init: change one match arm (lines 57-60)
│   ├── error.rs      → remove ParentProjectExists variant + exit-code mapping
│   └── mod.rs        → no changes
├── core/
│   └── manifest.rs   → no changes (already correct)
└── ...

README.md             → add nested projects example

Cargo.toml            → bump 0.3.1 → 0.3.2
```

## Key Changes

### `src/cli/commands.rs::cmd_init` (lines 57-60)

Before:
```rust
Ok((_, found_dir)) => {
    return Err(CliError::ParentProjectExists(found_dir.display().to_string()));
}
```

After:
```rust
Ok((_, found_dir)) => {
    // A parent has envy.toml, but the cwd does not.
    // Proceed with init — nested projects are supported (spec 014).
}
```

### `src/cli/error.rs` — remove `ParentProjectExists`

Lines 35-37 deleted:
```rust
/// `init` was run inside a directory tree that already has a parent project.
#[error("parent project detected: \"{0}\" already contains envy.toml")]
ParentProjectExists(String),
```

Line 132 deleted from `cli_exit_code`:
```rust
CliError::ParentProjectExists(_) => 3,
```

## Test Strategy

### New unit tests in `src/cli/commands.rs`

| Test | What it verifies | FR/SC |
|------|-----------------|-------|
| `init_nested_succeeds` | Parent project exists, init in subdirectory succeeds with different UUID | FR-001, SC-001, SC-002 |
| `init_already_initialised_still_rejected` | Init in CWD with existing `envy.toml` returns `AlreadyInitialised` | FR-002, SC-003 |

### Pre-existing test verified

- `find_manifest_in_parent_dir` in `src/core/manifest.rs` — already verifies closest-ancestor walker. Continues to pass.

## Documentation

| File | Change |
|------|--------|
| `README.md` | Add nested projects example in Quickstart section. |
| `Cargo.toml` | Bump `0.3.1` → `0.3.2` (patch). |

## Release Impact

- Version: `0.3.1` → `0.3.2` (PATCH per spec).
- cargo-dist: picks up new tag automatically.
- No CI workflow changes.

## Complexity Tracking

No violations.
