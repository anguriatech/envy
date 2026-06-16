# Quickstart: Implementing Nested Envy Projects

**Feature**: 014-nested-projects
**Date**: 2026-06-10

## Pre-flight

1. Make sure you are on the `014-nested-projects` branch:
   ```bash
   git branch --show-current
   ```

## Files to modify (in order)

1. `src/cli/commands.rs` — change one match arm in `cmd_init` (lines 57-60).
2. `src/cli/error.rs` — remove `CliError::ParentProjectExists` variant + exit-code mapping.
3. `src/cli/commands.rs` (tests) — add 2 new unit tests.
4. `README.md` — add nested projects note.
5. `Cargo.toml` — bump version 0.3.1 → 0.3.2.

## Implementation

### Step 1: `src/cli/commands.rs` — change the match arm (lines 57-60)

Replace:
```rust
Ok((_, found_dir)) => {
    return Err(CliError::ParentProjectExists(
        found_dir.display().to_string(),
    ));
}
```
with:
```rust
Ok((_, found_dir)) => {
    // A parent has envy.toml, but the cwd does not.
    // Proceed with init — nested projects are supported (spec 014).
}
```

Also update the doc comment at line 46 removing the `ParentProjectExists` reference:
```
/// - [`CliError::AlreadyInitialised`] — `envy.toml` exists in the cwd.
/// - [`CliError::VaultOpen`] — keyring, vault open, or DB write failed.
```

### Step 2: `src/cli/error.rs` — remove `ParentProjectExists`

Remove lines 35-37:
```rust
/// `init` was run inside a directory tree that already has a parent project.
#[error("parent project detected: \"{0}\" already contains envy.toml")]
ParentProjectExists(String),
```

Remove line 132 from `cli_exit_code`:
```rust
CliError::ParentProjectExists(_) => 3,    // DELETE THIS LINE
```

Verify: `grep -r ParentProjectExists src/` returns zero matches.

### Step 3: Tests in `src/cli/commands.rs` test module

Add two new tests in the `mod tests` block:

1. `init_nested_succeeds`:
   - Creates a parent project via `cmd_init()` in a tempdir.
   - Creates a subdirectory `child` inside the tempdir.
   - `cd`s to the subdirectory, runs `cmd_init()` again.
   - Asserts exit 0.
   - Reads both `envy.toml` files and asserts the `project_id` fields are different.

2. `init_already_initialised_still_rejected`:
   - Creates a project via `cmd_init()` in a tempdir.
   - Runs `cmd_init()` again from the same directory.
   - Asserts `Err(CliError::AlreadyInitialised)`.

The tests do not need the `ENV_LOCK` mutex (they don't touch env vars), but they do need a real vault at `~/.envy/vault.db`. If the test environment does not have a keyring, the CI fallback (`ENVY_PASSPHRASE` or `CI` env var) may be needed.

### Step 4: `README.md`

Add a note in the Quickstart section:
```markdown
#### Nested projects (monorepo / multi-project support)

Since v0.3.2, `envy init` works in subdirectories of existing envy projects.
Each project gets its own UUID in the vault and its own `envy.toml` + `envy.enc`.
Commands resolve the closest `envy.toml` automatically — running `envy list`
from a child directory shows the child's secrets, not the parent's.

    /monorepo/
      envy.toml     ← org-wide credentials (UUID: a1b2c3d4)
      envy.enc
      /project-a/
        envy init   ← project-specific credentials (UUID: e5f6a7b8)
        envy.toml
        envy.enc
      /project-b/
        envy init   ← different UUID again
        envy.toml
        envy.enc
```

### Step 5: `Cargo.toml`

Bump from `0.3.1` to `0.3.2`.

## Verification

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
grep -r ParentProjectExists src/  # Must return zero matches (exit 1)
```
