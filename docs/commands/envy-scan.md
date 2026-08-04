# envy scan

Scan the project's working tree for plaintext copies of vault secrets.

## What it does

Compares every **exact** secret value already stored in the vault against the
contents of files in the working tree, reporting any plaintext copy. This is
not a generic pattern-based scanner (like gitleaks/trufflehog) — it knows the
actual secrets. Values are masked by default; `--reveal` shows the matched
value with a stderr warning. Exit code 0 = clean, 1 = leak(s) found, 2+ =
error — safe to gate a pre-commit hook on.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy scan [-e ENV] [--reveal]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Restrict the scan to secrets from one environment (default: all) |
| `--reveal` | Show the matched plaintext value (stderr warning emitted first) |

## Examples

```bash
envy scan
envy scan -e production --reveal
envy scan || echo "leaks found — fix before committing"
```

## How it works

Decrypts the vault's secrets, walks the working tree (respecting
`.gitignore`; dotfiles are deliberately scanned), and matches each secret's
exact bytes against file contents. Files larger than 10 MiB are skipped.
Matches are reported with the file path and line — masked by default. The
leak is never written anywhere: output only, no state changes.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Clean — no plaintext copies found |
| `1` | Leak(s) found (diff(1) convention — not an error) |
| `2+` | Error |

## Related commands

- [envy hooks](envy-hooks.md) — install a pre-commit hook that runs `envy scan`
- [envy migrate](envy-migrate.md) — import an existing `.env` into the vault
- [envy run](envy-run.md) — the plaintext-free alternative to `.env` loading
