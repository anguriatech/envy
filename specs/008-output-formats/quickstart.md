# Quickstart: Machine-Readable Output Formats

**Feature**: 008-output-formats

Five integration scenarios, each independently testable.

---

## Scenario 1 — Parse `envy list` in a Shell Script

```bash
# Set up
envy init
envy set API_KEY=abc123
envy set DB_HOST=localhost

# Get JSON output and parse with jq
envy list --format json | jq -r '.secrets[] | .value'
# Output:
# abc123
# localhost

# Short form
envy list -f json | jq '.secrets | length'
# Output: 2
```

**Validates**: FR-001, FR-002, FR-004, SC-001

---

## Scenario 2 — Source Secrets into the Current Shell

```bash
envy init
envy set DB_PASS=s3cr3t
envy set SPECIAL="it's here"

# Dotenv file for Docker Compose
envy export --format dotenv > .env
cat .env
# DB_PASS=s3cr3t
# SPECIAL=it's here

# Source into shell
eval "$(envy export --format shell)"
echo "$DB_PASS"   # s3cr3t
echo "$SPECIAL"   # it's here
```

**Validates**: FR-006, FR-007, FR-008, FR-009, SC-002

---

## Scenario 3 — VS Code Extension reads a single secret

```bash
envy init
envy set API_KEY=my-token

# Found
envy get API_KEY --format json
# {"key":"API_KEY","value":"my-token"}

# Not found — exit 1
envy get MISSING_KEY --format json; echo "exit: $?"
# {"error":"key not found"}
# exit: 1
```

**Validates**: FR-005, SC-005

---

## Scenario 4 — Backward Compatibility

```bash
envy init
envy set FOO=bar

# Without --format: identical to before this feature
envy list        # FOO
envy get FOO     # bar
```

**Validates**: FR-011, SC-003

---

## Scenario 5 — Invalid Format Value

```bash
envy list --format xml
# error: invalid value 'xml' for '--format <FORMAT>'
#   [possible values: table, json, dotenv, shell]
# exit code: 2

echo $?   # 2
```

**Validates**: FR-012
