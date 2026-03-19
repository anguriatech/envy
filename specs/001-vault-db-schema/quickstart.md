# Quickstart: Vault Schema — Build & Validate

**Feature**: 001-vault-db-schema
**Date**: 2026-03-18

---

## Prerequisites

- Rust stable toolchain (`rustup show`)
- `cargo-audit` installed (`cargo install cargo-audit`)
- Linux: `libsecret-1-dev` (or equivalent) for `keyring` crate OS integration
- macOS: Xcode Command Line Tools (Keychain access)

---

## 1. Initialize the Cargo project

```bash
cargo new envy --bin
cd envy
```

Add to `Cargo.toml`:

```toml
[dependencies]
rusqlite  = { version = "0.31", features = ["bundled-sqlcipher"] }
uuid      = { version = "1",    features = ["v4"] }
keyring   = "2"

[dev-dependencies]
tempfile  = "3"  # for isolated test vault files
```

> **Critical**: The `bundled-sqlcipher` feature compiles SQLCipher statically.
> Do NOT use the plain `bundled` feature — that compiles plaintext SQLite.

---

## 2. Verify SQLCipher is active

```bash
cargo build 2>&1 | grep -i cipher
# Expected: output referencing sqlcipher during link phase
```

At runtime, verify encryption is active:

```rust
// In a test: open vault, check PRAGMA cipher_version returns a value
let version: String = conn.query_row(
    "PRAGMA cipher_version",
    [],
    |r| r.get(0),
)?;
assert!(!version.is_empty(), "SQLCipher must be active");
```

---

## 3. Run schema migrations (smoke test)

```bash
cargo test db::tests::test_schema_creation -- --nocapture
```

Expected output:
```
✓ projects table created
✓ environments table created
✓ secrets table created
✓ PRAGMA user_version = 1
✓ PRAGMA foreign_keys = ON verified
✓ PRAGMA journal_mode = WAL verified
```

---

## 4. Verify defense-in-depth encryption

```bash
cargo test db::tests::test_secret_is_not_plaintext -- --nocapture
```

What the test does:
1. Opens a vault, stores `("STRIPE_KEY", "sk_test_supersecret")`.
2. Closes the vault connection.
3. Opens the `.db` file as raw bytes and asserts `"sk_test_supersecret"` is NOT present
   in the byte stream.

---

## 5. Verify cascade deletes

```bash
cargo test db::tests::test_cascade_deletes -- --nocapture
```

What the test does:
1. Creates a project → 2 environments → 3 secrets each (6 secrets total).
2. Deletes the project.
3. Asserts `environments` and `secrets` tables are empty.

---

## 6. Verify referential integrity

```bash
cargo test db::tests::test_foreign_key_enforcement -- --nocapture
```

What the test does:
1. Attempts to insert an environment with a non-existent `project_id`.
2. Asserts `DbError::ConstraintViolation` is returned (not a panic).

---

## 7. Security audit

```bash
cargo audit
# Expected: 0 vulnerabilities
```

Run before any commit touching `Cargo.toml` or `Cargo.lock`.
