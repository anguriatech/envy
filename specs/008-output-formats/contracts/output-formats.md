# CLI Output Contract: `--format` Flag

**Feature**: 008-output-formats
**Version**: 1.0
**Status**: Draft

This document defines the stable, machine-readable output shapes that consumers (scripts, extensions, CI pipelines) can rely on. Any breaking change to these shapes requires a major version bump.

---

## Global Flag

```
envy [--format <table|json|dotenv|shell>] <command> [args]
     [-f      <table|json|dotenv|shell>]
```

- Default for all commands except `export`: `table`
- Default for `export`: `dotenv`
- Invalid value: exit code 2, error message listing accepted values

---

## `envy list [--format F]`

### `--format table` (default)

```
API_KEY
DATABASE_URL
JWT_SECRET
```

One key per line, alphabetical order. Empty vault → `(no secrets in <env>)` on **stderr**, exit 0.

### `--format json`

```json
{"secrets":[{"key":"API_KEY","value":"abc123"},{"key":"DB_HOST","value":"localhost"}]}
```

Empty vault:
```json
{"secrets":[]}
```

### `--format dotenv`

```
API_KEY=abc123
DB_HOST=localhost
```

### `--format shell`

```
export API_KEY='abc123'
export DB_HOST='localhost'
```

Single quotes in values are escaped using POSIX `'\''` technique.

---

## `envy get KEY [--format F]`

### `--format table` (default)

```
abc123
```

Raw value only, newline-terminated. Key not found: exit code 1, nothing on stdout.

### `--format json` — found

```json
{"key":"API_KEY","value":"abc123"}
```

Exit code: 0

### `--format json` — not found

```json
{"error":"key not found"}
```

Exit code: 1

---

## `envy export ENV [--format F]`

### `--format dotenv` (default)

```
API_KEY=abc123
DB_HOST=localhost
JWT_SECRET=supersecret
```

### `--format shell`

```
export API_KEY='abc123'
export DB_HOST='localhost'
export JWT_SECRET='supersecret'
```

### `--format json`

```json
{"environment":"production","secrets":[{"key":"API_KEY","value":"abc123"}]}
```

### `--format table`

Renders same as `--format dotenv` (export has no table representation; dotenv is the natural table for secrets).

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Key not found (get), or other runtime error |
| 2    | Invalid `--format` value (clap validation error) |
