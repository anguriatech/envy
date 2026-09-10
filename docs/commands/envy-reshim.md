# envy reshim

Generate command shims for the current project's toolchains.

## What it does

Detects `package.json`, `Cargo.toml`, and friends in the project root and
creates `~/.envy/shims/<cmd>` (plus `.cmd` twins on Windows) delegating to
`envy exec`. Run it once per project after `envy auto on`; re-run after adding
a toolchain. Shims only inject in opted-in projects — elsewhere they pass
commands through untouched.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy reshim [--prune]
```

| Flag | Description |
|------|-------------|
| `--prune` | Remove auto-generated shims no longer detected (manually added ones are kept) |

Detected by default: `package.json` → npm, npx, node · `yarn.lock` → yarn ·
`pnpm-lock.yaml` → pnpm · `Cargo.toml` → cargo · `pyproject.toml` → python, uv ·
`requirements.txt`/`setup.py` → python, pip · `go.mod` → go · `Makefile` →
make · `justfile` → just · `Gemfile` → bundle · `composer.json` → composer ·
`pom.xml` → mvn · `build.gradle` → gradle · `mix.exs` → mix, elixir ·
`Taskfile.yml` → task · `deno.json` → deno · `bun.lockb` → bun ·
`compose.yaml` → docker, docker-compose.

Rule: a trigger must reliably imply the command. A bare `Dockerfile` is
deliberately not a trigger: its usage is dominated by `docker build`, where
injected env can leak into image layers. Compose files imply runtime usage
(`up`/`run`), so they are covered — and `envy shim add docker` remains
possible for anything else. Cover project-specific CLIs (`expo`, `vercel`,
…) the same way: one `shim add` and done.

## Examples

```bash
cd my-project          # has package.json
envy auto on
envy reshim
#   reshim: created 3: npm, npx, node

# Later, after removing Node from the project:
envy reshim --prune
#   reshim: pruned npm
#   reshim: pruned npx
#   reshim: pruned node
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Reads trigger files in the manifest directory and writes dumb shims (an
`exec envy exec …` shell script on Unix, a 2-line `.cmd` on Windows) with a
provenance header (`auto` vs `manual`). Existing files are never overwritten;
`--prune` deletes only `auto` shims outside the current detection set.
Manifest read + filesystem only — no vault or keyring access, so it works on
cold machines and in CI.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | No `envy.toml` found (run `envy init` first) or unwritable shims dir |

## Related commands

- [envy auto](envy-auto.md) — opt the project in first (`auto on`)
- [envy shim](envy-shim.md) — cover commands detectors miss (`shim add`)
- [envy doctor](envy-doctor.md) — verify shims are on `PATH` and first
- [envy shell-init](envy-shell-init.md) — the one-time `PATH` line shims need
