# Monorepo — independent secrets per nested project

Since v0.3.2, `envy init` works in subdirectories of existing envy
projects. Each project gets its own UUID in the vault and its own
`envy.toml` + `envy.enc`. Commands resolve the **closest** `envy.toml`
automatically — running `envy list` from `apps/app-a/` shows app-a's
secrets, not the parent's.

```
repo/
├── envy.toml            ← team-wide secrets
└── apps/
    ├── app-a/
    │   └── envy.toml    ← app-a-specific secrets
    └── app-b/
        └── envy.toml    ← app-b-specific secrets (different UUID)
```

## Running the demo

```bash
cd examples/monorepo
ENVY_BIN=./target/debug/envy ./script.sh
```

`script.sh` recreates the tree above in a fresh temp dir, runs `envy init`
in each project, sets independent dummy secrets, and verifies that each
project resolves **only its own** secrets.

> **Note on `envy.toml.example`**: real manifests are never committed in
> `examples/` — a committed `envy.toml` would make `envy init` fail with
> exit code 3 (project already exists). `envy init` generates the real file
> for you.

## What to try next

- [envy set](../../docs/commands/envy-set.md) with `-e ENV` — per-environment
  secrets inside each project.
- `examples/team-sync/` — sync a nested project's `envy.enc` independently.
