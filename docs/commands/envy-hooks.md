# envy hooks

Install a git hook that guards commits against leaked secrets.

## What it does

Installs a `pre-commit` hook in `.git/hooks` that runs `envy scan` on staged
files and **blocks the commit** if a vault secret's plaintext value is found.
It also prints a non-blocking warning when `envy status` shows unsealed drift.
Pure local git tooling — no network or CI dependency. Nothing leaves the
machine.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy hooks install [--force]
```

| Flag | Description |
|------|-------------|
| `--force` | Overwrite an existing, non-envy pre-commit hook (backed up first) |

## Examples

```bash
envy hooks install
envy hooks install --force   # replace a hand-written hook (backup kept)
```

## How it works

Writes the `pre-commit` hook script into `.git/hooks`. `envy hooks install`
refuses to overwrite a pre-existing hook that envy didn't install — unless
`--force` is given, in which case the existing file is backed up to
`pre-commit.envy-backup` first, never silently discarded. On commit, the hook
scans staged files for exact vault secrets and exits non-zero on a match.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `3` | Hook conflict — existing non-envy hook (use `--force`) |

## Related commands

- [envy scan](envy-scan.md) — the check the hook runs
- [envy audit](envy-audit.md) — see who touched what
- [envy init](envy-init.md) — create the project before installing hooks
