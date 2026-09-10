# envy doctor

Diagnose the shim setup: `PATH`, order, and coverage.

## What it does

The antidote to silent misconfiguration — the one failure mode shims add over
`envy run`. Checks that `~/.envy/shims` is on `PATH` and before version
managers that prepend (fnm/nvm/volta/mise/…), that the current opted-in
project has its shims generated, and that given commands actually resolve to
their shims. Exit 0 is clean, exit 1 lists findings with fixes. Never touches
the vault.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy doctor [CMD...]
```

| Argument | Description |
|----------|-------------|
| `[CMD...]` | Optionally check coverage for these commands too |

## Examples

```bash
envy doctor
#   ✓ shims dir on PATH.
#   ✓ shims precede version managers.
#   ✓ project shims up to date.
#   doctor: all checks passed.

envy doctor npm expo
#   ✓ 'npm' resolves to envy shims.
#   ✗ 'expo' resolves to /opt/homebrew/bin/expo — runs WITHOUT secrets.
#     fix: `envy shim add expo` or `envy run -- …`.
#   doctor: 1 problem(s) found.
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Filesystem + manifest-flag reads only (safe on cold machines, no keyring):
**C1** shims dir present on `PATH`; **C2** no known prepender (fnm, nvm,
volta, mise, rbenv, pyenv, asdf) sits above it; **C3** inside an opted-in
project, every detected command has a shim (else: run `reshim`); **C4** each
named command's first `PATH` hit is the shim, another binary (naked run), or
missing. Findings print their fix; only the exit code is machine-readable.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | All checks passed |
| `1` | One or more findings (details on stdout) |

## Related commands

- [envy shell-init](envy-shell-init.md) — the `PATH` line C1 asks for
- [envy reshim](envy-reshim.md) — fixes C3 findings
- [envy shim](envy-shim.md) — fixes C4 findings
- [envy exec](envy-exec.md) — what covered commands run through
