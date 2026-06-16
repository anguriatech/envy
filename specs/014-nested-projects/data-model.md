# Data Model: Allow Nested Envy Projects

**Feature**: 014-nested-projects
**Date**: 2026-06-10

## Entities

This feature does not introduce new persistent entities. It relaxes a single validation rule on one existing operation.

### Project UUID (existing, behaviour-relaxed)

| Field | Type | Notes |
|-------|------|-------|
| `project_id` | UUIDv4 string | Stored in `envy.toml`'s `project_id` field. Every directory with an `envy.toml` has its own UUID. Nested projects are simply two different UUIDs sharing the same vault file and the same directory tree hierarchy. |

**Behaviour relaxed by this feature**: `envy init` no longer requires that no ancestor contains `envy.toml`. The sole remaining reject condition is "the cwd itself has `envy.toml`" (`AlreadyInitialised`). A parent `envy.toml` is treated the same as "no manifest found at all" — the init proceeds and creates a new UUID.

**Invariants preserved**:
- The vault (`~/.envy/vault.db`) is shared across all projects on a machine, differentiated by UUID. This is unchanged — it was already true for multiple non-nested projects.
- `find_manifest` returns the closest ancestor `envy.toml`. For the init command, this is only used to distinguish "cwd has it" from "some ancestor has it". For all other commands, the closest ancestor is the project context.
- `create_manifest` writes `envy.toml` to the cwd. This is already correct for nested projects.

### Manifest Resolution (existing, verified)

The `find_manifest` walker from `src/core/manifest.rs` walks upward from the cwd, returning the first `envy.toml` found. No change needed:

```
/carpetaPadre/           ← find_manifest("/carpetaPadre") → returns /carpetaPadre/envy.toml
  envy.toml
  /proyecto1/
    envy.toml             ← find_manifest("/carpetaPadre/proyecto1") → returns /carpetaPadre/proyecto1/envy.toml
    /src/
                          ← find_manifest("/carpetaPadre/proyecto1/src") → returns /carpetaPadre/proyecto1/envy.toml
```

## State Transitions

```
                          ┌──────────────────────┐
                          │ No envy.toml in cwd; │
                          │ parent has envy.toml │
                          └──────────┬───────────┘
                                     │
                                     │ envy init (v0.3.2+)
                                     ▼
                          ┌──────────────────────┐
                          │ envy.toml created in │
                          │ cwd with a NEW UUID  │
                          │ (different from      │
                          │  parent's UUID)      │
                          └──────────────────────┘
```

**Failure transition** (unchanged):
- Cwd already has `envy.toml` → `AlreadyInitialised` error, exit 3, no file written.
