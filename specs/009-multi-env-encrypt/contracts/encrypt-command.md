# Contract: `envy encrypt` Command

**Feature**: 009-multi-env-encrypt
**Layer**: CLI → user-facing behaviour contract

---

## Passphrase Resolution Order (all modes)

For each environment being encrypted, the passphrase is resolved in this exact priority:

| Priority | Source | Trigger condition |
|----------|--------|-------------------|
| 1 | `ENVY_PASSPHRASE_<ENV>` | Env var set and non-whitespace (`<ENV>` = uppercase + hyphens→underscores) |
| 2 | `ENVY_PASSPHRASE` | Env var set and non-whitespace (no env-specific var found) |
| 3 | Interactive prompt | Neither env var is set |

If either env var is set but whitespace-only → **immediate exit 2** with error message. No fallthrough to interactive.

---

## Headless Mode

**Condition**: At least one `ENVY_PASSPHRASE*` variable is set and non-whitespace for any vault environment.

**Behaviour**:
- Interactive selection menu is NOT shown.
- All vault environments are iterated. Each env resolves its passphrase individually (priority table above).
- Environments with no resolvable passphrase are silently skipped.
- Pre-flight check is NOT performed.
- Diceware suggestion is NOT shown.
- New-environment confirmation (double-entry) is NOT required.

---

## Interactive Mode

**Condition**: No `ENVY_PASSPHRASE*` variable is set for ANY vault environment.

**Behaviour**:
1. `MultiSelect` menu shows all vault environments; user selects subset.
2. If zero selected → print `"Nothing to encrypt."`, exit 0.
3. For each selected environment:
   - **Existing envelope** (already in `envy.enc`):
     - Single-entry passphrase prompt (no Diceware suggestion).
     - Pre-flight check: attempt decryption with provided passphrase.
     - If mismatch → rotation warning + `Confirm` (default No).
   - **New envelope** (not in `envy.enc`):
     - Diceware passphrase suggested (4 words, CSPRNG).
     - User may accept (Enter) or type own passphrase.
     - Double-entry confirmation required.
     - If accepted suggestion → "SAVE THIS NOW" banner printed to stderr.

---

## Smart Merge Contract

After all selected environments are sealed:
- Envelopes **not selected** in this run are copied from the existing `envy.enc` byte-for-byte.
- Envelopes **selected** are replaced with the newly sealed version.
- The output `envy.enc` always contains the union of (existing envelopes) ∪ (newly sealed envelopes).

---

## Atomic Write Contract

1. New JSON is written to `envy.enc.tmp` (sibling file, same directory).
2. `envy.enc.tmp` is renamed to `envy.enc`.
3. A crash between steps 1 and 2 leaves the previous `envy.enc` intact.

---

## Exit Codes

| Code | Condition |
|------|-----------|
| 0 | Success (including "Nothing to encrypt") |
| 1 | `envy.enc` exists but is invalid JSON (FR-013) |
| 2 | Passphrase env var set but whitespace-only |
| 2 | Interactive prompt failed (no TTY, Ctrl-C) |
| 4 | Vault read error or file write failure |

---

## Error Messages (stable, machine-parseable)

| Condition | Message |
|-----------|---------|
| `ENVY_PASSPHRASE_X` is whitespace | `ENVY_PASSPHRASE_X is set but contains only whitespace` |
| No TTY and no env var | `passphrase prompt failed: …` (error from dialoguer) |
| `envy.enc` is malformed JSON | `envy.enc is malformed: …` |
| Atomic rename failed | `atomic rename failed: …` |
| No environments in vault | `No environments found. Use 'envy set' to add secrets first.` |
| Zero selected interactively | `Nothing to encrypt.` |
