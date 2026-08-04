# envy rotate

Re-seal an existing envelope in `envy.enc` with a new passphrase.

## What it does

Safely rotates an environment's envelope passphrase: the current passphrase
is verified **before** the new one is accepted, preventing the silent
key-rotation `envy encrypt` could otherwise perform in headless mode. The
rotation is forward-only — the old passphrase can no longer decrypt the
artifact. This is the safe path for periodic key rotation.

## Aliases

| Alias | Notes |
|-------|-------|
| None | — |

## Syntax & flags

```text
envy rotate [-e ENV]
```

| Flag | Description |
|------|-------------|
| `-e, --env <ENV>` | Environment to rotate (default: MultiSelect prompt from `envy.enc`) |

Interactive mode prompts for current, new, and confirmation passphrases.
Headless mode (CI) reads `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW`.

## Examples

```bash
envy rotate -e production
ENVY_PASSPHRASE_PRODUCTION='old-pass' ENVY_PASSPHRASE_PRODUCTION_NEW='new-pass' \
  envy rotate -e production
```

> **Note**: Examples use dummy values only — never commit real secrets.

## How it works

Unseals the existing envelope with the current passphrase (failure aborts —
no rotation on wrong passphrase), then re-seals with the new passphrase
derived key and replaces the envelope inside `envy.enc`. The old passphrase
immediately stops working. Passphrase inputs are kept in `Zeroizing`
containers.

**Exit codes**:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `2` | Wrong current passphrase / empty input |
| `3` | Environment not found in the artifact |
| `5` | `envy.enc` missing or unreadable |

## Related commands

- [envy encrypt](envy-encrypt.md) — strict sealing (refuses silent rotation)
- [envy status](envy-status.md) — rotation reminders per environment
- [envy diff](envy-diff.md) — verify vault ↔ artifact consistency after rotation
