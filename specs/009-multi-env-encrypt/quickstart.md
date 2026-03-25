# Quickstart: Multi-Environment Encryption

**Feature**: 009-multi-env-encrypt

---

## Scenario 1 — Headless CI (single passphrase, all environments)

```bash
# Seal all vault environments with one shared passphrase
export ENVY_PASSPHRASE="correct-horse-battery-staple"
envy encrypt
# → envy.enc written with all environments sealed
# → Exit 0, no prompts
```

## Scenario 2 — Headless CI (per-environment passphrases)

```bash
# Seal development with one passphrase, production with another
export ENVY_PASSPHRASE_DEVELOPMENT="dev-secret-pass"
export ENVY_PASSPHRASE_PRODUCTION="prod-secret-pass"
envy encrypt
# → envy.enc written with both environments sealed independently
# → Exit 0, no prompts
```

## Scenario 3 — Smart merge (preserve teammate's envelope)

```bash
# envy.enc already has 'production' sealed by a teammate
# Seal only 'development' without touching 'production'
export ENVY_PASSPHRASE_DEVELOPMENT="my-dev-pass"
envy encrypt
# → envy.enc now has both 'development' (new) and 'production' (unchanged)
```

## Scenario 4 — Interactive selection with Diceware suggestion

```bash
envy encrypt
# Checkbox menu appears:
#   [x] development
#   [ ] production
#   [x] staging
# → User selects development and staging

# For 'development' (new environment, no existing envelope):
#   Suggested: "correct-horse-battery-staple"
#   [press Enter to accept, or type your own]
# → User presses Enter
# → "SAVE THIS NOW" banner printed to stderr

# For 'staging' (existing envelope, passphrase matches):
#   → No rotation warning, proceed

# → envy.enc updated with new development and staging envelopes
```

## Scenario 5 — Key rotation with confirmation

```bash
envy encrypt
# → User selects 'production'
# → Enters a new passphrase (different from the one that sealed it)
# ⚠  Passphrase does not match existing data for 'production'.
#    Continuing will ROTATE the key.
# Continue and rotate key? [y/N]: y
# → production envelope replaced with new passphrase
```

## Scenario 6 — Hyphenated environment name

```bash
# Environment named 'my-env' in the vault
export ENVY_PASSPHRASE_MY_ENV="secret"
envy encrypt
# → 'my-env' resolved via ENVY_PASSPHRASE_MY_ENV (normalised)
```

## Scenario 7 — Abort on malformed envy.enc

```bash
echo "not json" > envy.enc
envy encrypt
# error: envy.enc is malformed: expected value at line 1 column 1
# Exit 1
```
