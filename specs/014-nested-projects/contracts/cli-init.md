# CLI Contract: `envy init` (Nested Projects Relaxation)

**Feature**: 014-nested-projects
**Date**: 2026-06-10
**Type**: CLI subcommand contract (replacement for §1.1 of the existing init contract)

## Synopsis

```
envy init
```

## Description

Initialises Envy in the current directory. Creates `envy.toml` (the project manifest) and registers a new project in the vault with a fresh UUID.

**Relaxed behaviour (v0.3.2+)**: ancestors that contain `envy.toml` no longer block initialisation. A parent project is treated as a sibling — the init creates a new project with its own UUID. This supports monorepo / multi-project use cases where separate directories need separate secrets.

## Reject Conditions

| Condition | Error | Exit code |
|-----------|-------|-----------|
| `envy.toml` exists in the CWD itself | `AlreadyInitialised` | 3 |
| I/O error (e.g., cannot read current directory, cannot write `envy.toml`, cannot open vault) | `VaultOpen` | 4 |

## Behavioural Change Table

| Scenario | Old (v0.3.1) | New (v0.3.2) |
|----------|---------------|----------------|
| Init in CWD without `envy.toml`, no ancestor has it | succeed | unchanged |
| Init in CWD without `envy.toml`, ancestor has it | **rejected** (`ParentProjectExists`) | **succeed** (new UUID, new vault row) |
| Init in CWD WITH `envy.toml` | rejected (`AlreadyInitialised`) | unchanged |

## Example

```bash
$ mkdir -p /monorepo/project-a
$ cd /monorepo && envy init
✓ Initialised envy project a1b2c3d4-....

$ cd /monorepo/project-a && envy init    # v0.3.1: rejected with "parent project detected"
✓ Initialised envy project e5f6a7b8-.... # v0.3.2: succeeds with different UUID

$ envy set API_KEY=sk_test_123          # stores under child's UUID
$ cd /monorepo && envy set CI_TOKEN=ghp_... # stores under parent's UUID
```

## What was removed

The `ParentProjectExists` error variant is removed from the `CliError` enum. It is no longer returned by any code path and no longer maps to any exit code.

## Out of Contract

- The vault structure is unchanged (one row per UUID).
- `envy.toml` design is unchanged (single `project_id` field).
- No recursive operations (no `envy list --all` or `envy encrypt --recursive`).
