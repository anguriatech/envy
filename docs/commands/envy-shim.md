# envy shim

Manage command shims manually (global, no project needed).

## What it does

Covers the long tail `reshim` detectors miss: `envy shim add expo` creates a
shim for any command by name. `list` shows what's installed, `rm` removes it.
Manually added shims survive `reshim --prune`. Works from any directory —
shims live in `~/.envy/shims/`, not in projects.

## Aliases

| Alias | Notes |
|-------|-------|
| `remove` | Alias of `rm` |

## Syntax & flags

```text
envy shim add <NAME>
envy shim rm <NAME>
envy shim list
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Command name to shim (letters, digits, dot, dash, underscore — no paths) |

## Examples

```bash
envy shim add expo      # cover the Expo CLI in every project
#   ✓ shim added for 'expo'.

envy shim list
#   expo
#   npm
#   node
#   npx

envy shim rm expo
#   ✓ shim removed for 'expo'.
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Writes the same dumb delegating shim `reshim` generates, marked
`manual` so pruning never removes it. Existing files are kept (re-adding
marks them manual instead). Names are validated — anything that could escape
`~/.envy/shims/` is rejected with exit 2. No vault or keyring access.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | `rm` of a missing shim, or unwritable shims dir |
| `2` | Invalid shim name |

## Related commands

- [envy reshim](envy-reshim.md) — auto-detect shims per project
- [envy doctor](envy-doctor.md) — check a command resolves to its shim
- [envy exec](envy-exec.md) — what shims call under the hood
