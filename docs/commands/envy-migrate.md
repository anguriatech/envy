# envy migrate

Import secrets from a legacy `.env` file.

## What it does

Reads `KEY=VALUE` pairs line by line from an existing plaintext `.env` file
and stores them in the encrypted vault — the recommended migration path off
plaintext secret files. Comment lines (`#`) and blank lines are skipped;
malformed lines produce a warning but do not abort the import.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy migrate FILE [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Target environment (default: `development`) |

## Examples

```bash
envy migrate .env
envy migrate .env -e staging
```

## How it works

Parses the file as `KEY=VALUE` lines (the first `=` separates key and value),
encrypts each value with AES-256-GCM, and stores it in the vault. The
original plaintext file is left untouched — delete it yourself (and run
`envy scan` first to confirm no other plaintext copies exist).

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success (at least one secret imported) |
| `1` | File not found, or zero secrets imported |
| `2` | Invalid input / empty file |
| `4` | Vault or crypto failure |

## Related commands

- [envy init](envy-init.md) — initialise the project before migrating
- [envy scan](envy-scan.md) — find remaining plaintext copies
- [envy encrypt](envy-encrypt.md) — seal the imported secrets into `envy.enc`
