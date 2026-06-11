# CLI Contract: `envy rotate`

**Feature**: 012-cli-rotate
**Date**: 2026-06-10
**Type**: CLI subcommand contract (mirrors the style of `contracts/cli-sync.md` and `contracts/diff-command.md`)

## Synopsis

```
envy rotate [-e ENV]
```

## Description

Re-seals an existing envelope in `envy.enc` with a new passphrase, after verifying that the current passphrase matches the existing envelope. This is the safe path for key rotation; unlike `envy encrypt`, it never silently re-seals an envelope with a passphrase that does not match the existing one.

The command is purely additive — it does not modify the local vault. The vault remains the source of truth for secret values; the artifact is rebuilt from the vault and re-sealed with the new passphrase.

## Arguments

| Flag | Short | Required | Description |
|------|-------|----------|-------------|
| `--env` | `-e` | No | The environment to rotate. If omitted, the user is prompted to select one or more envelopes from those present in `envy.enc` (MultiSelect, same UX as `envy encrypt`). |

## Environment Variables

| Variable | Used for | Required for headless mode? |
|----------|----------|-----------------------------|
| `ENVY_PASSPHRASE_<ENV>` | Current passphrase for `<ENV>` | Yes (paired with `_NEW`) |
| `ENVY_PASSPHRASE_<ENV>_NEW` | New passphrase for `<ENV>` | Yes (paired without `_NEW`) |

Notes:
- `<ENV>` is the env name uppercased, with `-` replaced by `_` (e.g. `production` → `PRODUCTION`).
- The command does NOT honour the global `ENVY_PASSPHRASE` (without a per-env suffix) for the rotation. Rotation must be explicit.
- When BOTH `ENVY_PASSPHRASE_<ENV>` and `ENVY_PASSPHRASE_<ENV>_NEW` are set AND a TTY is present, headless mode is preferred (env vars are used, prompts are skipped). This matches `envy encrypt`'s behaviour.

## Interactive Prompts (TTY mode)

When no TTY is present and the required env vars are not set, the command exits with code 2 and a clear error.

When a TTY is present and the required env vars are not set, the prompts are:

1. `Passphrase for 'ENV'    (hidden input)` — the current passphrase.
2. `New passphrase for 'ENV'    (hidden input)` — the new passphrase.
3. `Confirm new passphrase    (hidden input)` — must match the new passphrase exactly.

For multi-environment rotation (no `-e` flag), the three prompts are repeated for each selected env in turn.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — at least one envelope rotated. |
| 1 | Missing `envy.enc` (no envelope to rotate). |
| 2 | Invalid input: wrong current passphrase, new = current, confirmation mismatch, whitespace-only new passphrase, no TTY and no env vars. |
| 3 | Env not found in `envy.enc` (use `envy encrypt -e ENV` first). |
| 4 | Vault or file write failure. |

## Output (stdout)

On success (single-env):

```
  ✓  'production' rotated. Passphrase changed.
     Previous passphrase can no longer decrypt this artifact.
```

The first line is a green check-mark (✓) followed by the env name and a one-line success message. The second line is a "forward-only" warning in plain text.

On success (multi-env), one block per rotated env, in the order they were processed. Envs that were skipped (empty vault, wrong current passphrase in interactive multi-mode) appear as a yellow warning.

On failure, the error message is printed to stderr prefixed with `error: `.

## Behavioural Contract

1. The command MUST verify the current passphrase by attempting to unseal the existing envelope before accepting the new passphrase.
2. The command MUST leave `envy.enc` and the vault unchanged on any input-related failure.
3. The command MUST write the updated `envy.enc` atomically.
4. The command MUST update the `sync_marker` for the rotated env (via the reused `seal_env` helper).
5. The command MUST NOT modify the local vault.
6. The command MUST preserve all other envelopes in `envy.enc` byte-identically.
7. The current and new passphrases MUST be wrapped in `zeroize::Zeroizing<String>` and dropped before any early return.

## Examples

### Happy path (interactive)

```bash
$ envy rotate -e production
Passphrase for 'production':
New passphrase for 'production':
Confirm new passphrase:
  ✓  'production' rotated. Passphrase changed.
     Previous passphrase can no longer decrypt this artifact.
```

### Headless path (CI)

```bash
$ ENVY_PASSPHRASE_PRODUCTION=old-pass \
  ENVY_PASSPHRASE_PRODUCTION_NEW=new-pass \
  envy rotate -e production
  ✓  'production' rotated. Passphrase changed.
     Previous passphrase can no longer decrypt this artifact.
```

### Wrong current passphrase

```bash
$ envy rotate -e production
Passphrase for 'production':
error: current passphrase does not match the existing envelope
$ echo $?
2
```

### Missing `envy.enc`

```bash
$ envy rotate -e production
error: file not found: /path/to/envy.enc
$ echo $?
1
```

### Env not in artifact

```bash
$ envy rotate -e nonexistent
error: environment 'nonexistent' not found in vault or artifact
$ echo $?
3
```

## Out of Contract (deferred)

- Rotation audit log.
- Auto-rotation scheduling.
- Revocation semantics.
- Changes to `envy encrypt`'s silent key-rotation behaviour (preserved for backward compatibility).
- Distribution of the new passphrase to the team.
- Removal of the existing `confirm_key_rotation` warning in `envy encrypt`.
- Changes to the `envy-vscode` extension (a follow-up spec will add "Envy: Rotate Passphrase").
