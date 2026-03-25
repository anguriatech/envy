# Quickstart & Integration Scenarios

**Feature**: `010-status-command`
**Date**: 2026-03-25

---

## Scenario 1: Developer checks sync state before committing

```bash
$ envy status

Vault: ~/.envy/vault.db  (project: my-app)

 Environment   Secrets   Last Modified      Status
 -----------   -------   ----------------   ----------------
 development       3     5 minutes ago      ⚠ Modified
 production        5     2 days ago         ✓ In Sync

Artifact: ./envy.enc  (last written: 2 days ago)
  Sealed environments: production
```

Developer sees `development` is Modified → runs `envy encrypt` before committing.

---

## Scenario 2: Status after a successful encrypt

```bash
$ envy encrypt
Sealed 1 environment(s) → ./envy.enc
  ✓  development

$ envy status

 Environment   Secrets   Last Modified      Status
 -----------   -------   ----------------   ----------------
 development       3     5 minutes ago      ✓ In Sync
 production        5     2 days ago         ✓ In Sync

Artifact: ./envy.enc  (last written: just now)
  Sealed environments: development, production
```

---

## Scenario 3: Fresh vault — no environments, no artifact

```bash
$ envy status
No environments found. Use 'envy set' to add secrets first.
Artifact: ./envy.enc  — not found
```

---

## Scenario 4: CI/CD pipeline gate — JSON output

```bash
$ envy status --format json | jq '.environments[] | select(.status != "in_sync") | .name'
"development"
```

A CI step can fail the build if any environment is not in sync:

```bash
NOT_SYNCED=$(envy status --format json | jq '[.environments[] | select(.status != "in_sync")] | length')
if [ "$NOT_SYNCED" -gt 0 ]; then
  echo "ERROR: Some environments are not in sync. Run 'envy encrypt' first."
  exit 1
fi
```

---

## Scenario 5: `envy.enc` missing — vault table still renders

```bash
$ envy status

 Environment   Secrets   Last Modified      Status
 -----------   -------   ----------------   ----------------
 production        5     1 hour ago         ✗ Never Sealed

Artifact: ./envy.enc  — not found
```

---

## Scenario 6: Malformed `envy.enc` — graceful degradation

```bash
$ echo "not-valid-json" > envy.enc
$ envy status

 Environment   Secrets   Last Modified      Status
 -----------   -------   ----------------   ----------------
 production        5     1 hour ago         ✗ Never Sealed

Artifact: ./envy.enc  — unreadable (malformed JSON)
```

---

## Scenario 7: Environment in artifact but not in local vault

```bash
$ envy status

 Environment   Secrets   Last Modified      Status
 -----------   -------   ----------------   ----------------
 development       3     1 hour ago         ✓ In Sync

Artifact: ./envy.enc  (last written: 1 hour ago)
  Sealed environments: development, staging
  ⚠  staging is in the artifact but not in the local vault
```

---

## End-to-End Test Assertions (for tasks.md)

| Scenario | Setup | Command | Assert |
|----------|-------|---------|--------|
| Never Sealed | vault with `development` env, no encrypt | `envy status` | output contains "Never Sealed" for development |
| In Sync | encrypt development, no further changes | `envy status` | output contains "In Sync" for development |
| Modified | encrypt, then `envy set NEW_KEY=val`, then status | `envy status` | output contains "Modified" for development |
| No artifact | no envy.enc | `envy status` | output contains "not found" for artifact section |
| JSON valid | any vault state | `envy status --format json` | output is valid JSON, status strings are lowercase |
| Exit 0 on no envs | empty project | `envy status` | exit code 0 |
| Sealed_at persists | encrypt, reopen vault, status | `envy status` | "In Sync" (sync marker survives vault reopen) |
