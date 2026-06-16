# Research: Allow Nested Envy Projects

**Feature**: 014-nested-projects
**Date**: 2026-06-10
**Status**: All clarifications resolved (2 clarifications provided by user in the planning input)

## Summary

Allow `envy init` in subdirectories that have a parent envy project. The change consists of two edits: one match arm in `cmd_init` and one variant removal in `CliError`. `find_manifest`, `create_manifest`, and all other commands already support nested projects correctly.

## Decisions

### Decision 1: Remove `CliError::ParentProjectExists` entirely (clarification #1)

**Decision**: Delete the `ParentProjectExists(String)` variant from `CliError` in `src/cli/error.rs`, along with its `#[error("...")]` Display impl and its `cli_exit_code` mapping (currently mapped to exit 3). Verify with `grep -r ParentProjectExists src/` returning zero matches after deletion.

**Rationale**:
- The variant is no longer returned by any code path after the `cmd_init` change.
- Keeping it as `#[deprecated]` would add an allow-attribute and a dead-code warning. The user explicitly chose deletion over deprecation.
- No external code pattern-matches on this variant (the error enum is consumed by `cli_exit_code` and `format_cli_error` only).
- The user's explicit wording: "remove the variant from CliError in src/cli/error.rs (not deprecated — just delete)."

**Alternatives considered**:
- *Keep as deprecated*: rejected by the user's clarification.
- *Keep as dead code with `#[allow(dead_code)]`*: rejected — adds unnecessary noise to the codebase and violates the "no dead code" principle in the project's Constitution.
- *Rename to `#[doc(hidden)] ParentProjectExists`*: rejected — same issue as deprecation, and the name is misleading when the variant is no longer returned.

**Implementation**: a single `edit` call to remove lines 35-37 from `src/cli/error.rs` (the variant definition + its doc comment + its `#[error]` attribute). Plus a removal of the exit-code mapping at `src/cli/error.rs:132`.

### Decision 2: The `cmd_init` change is exactly one match arm (clarification #2)

**Decision**: The second match arm in `cmd_init` (lines 57-60) currently returns `ParentProjectExists("...")`. It is changed to a no-op fall-through:

```rust
Ok((_, found_dir)) => {
    // A parent has envy.toml, but the cwd does not.
    // Proceed with init — nested projects are supported (spec 014).
}
```

This is the **entire** production change. `find_manifest`, `create_manifest`, and all other code paths are unchanged and already correct for nested projects.

**Rationale**:
- The ancestor's `envy.toml` is treated the same as "no manifest found" for the purpose of init. The init proceeds and creates a new `envy.toml` in the cwd.
- The `AlreadyInitialised` arm (lines 54-56) is unchanged — init still rejects double-init when the cwd itself has `envy.toml`.
- `find_manifest` already returns the closest ancestor correctly — no change needed. When the newly-created child `envy.toml` is written, subsequent `find_manifest` calls from the child's directory will return the child's manifest (closest ancestor).
- The user's explicit wording: "That is the entire production change. No other file is touched."

**Alternatives considered**:
- *Add a new match arm for "found in parent" vs "found in cwd"*: unnecessary — the existing arms already distinguish these two cases via the `if found_dir == cwd` guard.
- *Modify `find_manifest` to have an optional "init mode"*: rejected — adds complexity to a correct function. The init command is the right place for the policy decision.

### Decision 3: Two new unit tests, no existing tests to update

**Decision**: Add two new tests in the `mod tests` block of `src/cli/commands.rs`:

1. `init_nested_succeeds` — creates a parent project via `cmd_init` in `/tmp/test/`, then creates a subdirectory `/tmp/test/child/`, `cd`s into it, runs `cmd_init` again, asserts exit 0, reads both `envy.toml` files and asserts the UUIDs are different.
2. `init_already_initialised_still_rejected` — creates a project via `cmd_init`, runs `cmd_init` again from the same directory, asserts `Err(CliError::AlreadyInitialised)`.

There are no existing `cmd_init` tests in the `commands.rs` test module to update. The `find_manifest_in_parent_dir` test in `src/core/manifest.rs` continues to pass — it already verifies the closest-ancestor walker behaviour.

**Rationale**:
- `cmd_init` had zero test coverage in the unit test module before this spec. Adding tests is the right time.
- The tests use the existing `TEST_MASTER_KEY` and `ENV_LOCK` patterns. `cmd_init` does not require the `ENV_LOCK` mutex (it doesn't touch env vars), but the test infrastructure (e.g., `open_test_vault`) may need it for other operations in the same test.

### Decision 4: Documentation

**Decision**:
- `README.md`: add a note in the Quickstart/Project Structure section showing a nested project example.
- No other doc changes needed.

**Rationale**: The change is a single-behaviour relaxation. The README is the user-facing documentation. The developer-guide already describes `find_manifest`'s walker behaviour and does not mention `ParentProjectExists`.

### Decision 5: Version bump

**Decision**: `Cargo.toml` bumps from `0.3.1` to `0.3.2` (patch).

**Rationale**: The user's "Versioning" section explicitly specifies this. A behaviour relaxation on an existing command is a patch bump per the project's pre-1.0 versioning convention.
