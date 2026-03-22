# Quickstart: CLI Sync Commands (encrypt / decrypt)

**Feature**: 006-cli-sync-commands
**Date**: 2026-03-22

These scenarios serve as the acceptance test guide for the implementation. Each one must work end-to-end on a developer machine.

---

## Prerequisites

A working Phase 1 installation:
```bash
cd my-project/
envy init          # creates envy.toml
envy set STRIPE_KEY=sk_test_abc
envy set DATABASE_URL=postgres://dev -e development
envy set STRIPE_KEY=sk_live_xyz -e production
```

---

## Scenario 1: Seal vault for team sharing (Startup Mode)

```bash
envy encrypt
# Enter passphrase: ········
# Confirm passphrase: ········

# Output:
# Sealed 2 environment(s) → envy.enc
#   ✓  development   (2 secrets)
#   ✓  production    (1 secret)
```

`envy.enc` is now in the project directory. Commit it:
```bash
git add envy.enc
git commit -m "chore: update encrypted secrets"
```

---

## Scenario 2: Restore secrets after git pull (Startup Mode)

```bash
git pull
envy decrypt
# Enter passphrase: ········

# Output:
# Imported 2 environment(s) from envy.enc
#   ✓  development   (2 secrets upserted)
#   ✓  production    (1 secret upserted)
```

---

## Scenario 3: Short aliases

```bash
envy enc    # same as envy encrypt
envy dec    # same as envy decrypt
```

---

## Scenario 4: Seal only one environment

```bash
envy enc -e staging
# Sealed 1 environment(s) → envy.enc
#   ✓  staging   (3 secrets)
```

---

## Scenario 5: Progressive Disclosure (Enterprise Mode)

Two environments were sealed with different passphrases. Developer only has the dev key:

```bash
envy decrypt
# Enter passphrase: ········   (dev key)

# Output:
# Imported 1 environment(s) from envy.enc
#   ✓  development   (2 secrets upserted)
#   ⚠  production    skipped — different passphrase or key

# Exit code: 0  (not an error — partial access is the expected behaviour)
```

---

## Scenario 6: Headless CI/CD (GitHub Actions)

```yaml
# .github/workflows/deploy.yml
- name: Decrypt secrets
  env:
    ENVY_PASSPHRASE: ${{ secrets.ENVY_PASSPHRASE }}
  run: |
    envy decrypt
    envy run -e production -- ./deploy.sh
```

No terminal prompt is shown. The command reads `ENVY_PASSPHRASE` automatically.

---

## Scenario 7: Wrong passphrase (all skipped)

```bash
envy decrypt
# Enter passphrase: ········  (wrong key)

# stderr:
# error: no environments could be decrypted — check your passphrase

# Exit code: 1
```

---

## Scenario 8: Missing envy.enc

```bash
# No envy.enc in current project
envy decrypt

# stderr:
# error: envy.enc not found at /home/user/my-project/envy.enc

# Exit code: 1
```

---

## Scenario 9: Passphrase mismatch on encrypt

```bash
envy encrypt
# Enter passphrase: ········
# Confirm passphrase: ········  (different)
# Passphrases do not match.    (dialoguer retries automatically)
# Enter passphrase: ········   (user retries or Ctrl-C to abort)
```

If the user presses Ctrl-C:
```
# stderr:
# error: passphrase input failed: interrupted

# Exit code: 2
```

---

## Error Reference

| Scenario | Exit Code | Message |
|----------|-----------|---------|
| Success (all) | 0 | `Imported N environment(s) from envy.enc` |
| Success (partial) | 0 | `Imported N environment(s)` + skipped list |
| envy.enc not found | 1 | `error: envy.enc not found at <path>` |
| Nothing imported | 1 | `error: no environments could be decrypted — check your passphrase` |
| Empty passphrase | 2 | `error: passphrase must not be empty or whitespace` |
| Passphrase input fail | 2 | `error: passphrase input failed: <reason>` |
| Malformed envy.enc | 4 | `error: envy.enc is malformed: <reason>` |
| Unsupported version | 4 | `error: envy.enc has unsupported schema version <N>` |
| Vault error | 4 | `error: vault error: <reason>` |
