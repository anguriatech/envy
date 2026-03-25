# Implementation Plan: Machine-Readable Output Formats

**Branch**: `008-output-formats` | **Date**: 2026-03-24 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/008-output-formats/spec.md`

## Summary

Add a global `--format <table|json|dotenv|shell>` flag to the Envy CLI and a new `export` command. A dedicated presentation layer (`src/cli/format.rs`) decouples output formatting from command logic so that `cmd_list`, `cmd_get`, and `cmd_export` collect data into serialisable structs and delegate all rendering to `format.rs`. Existing `table` output is unchanged when the flag is omitted.

## Technical Context

**Language/Version**: Rust stable (edition 2024, MSRV 1.85)
**Primary Dependencies**: `clap` (derive API, `ValueEnum`), `serde` + `serde_json` (already present), `thiserror`
**Storage**: SQLite via `rusqlite` with `bundled-sqlcipher-vendored-openssl` (existing vault, read-only for this feature)
**Testing**: `cargo test` — unit tests in `format.rs`, integration tests in `tests/cli_integration.rs`
**Target Platform**: Linux, macOS, Windows (same as existing CI matrix)
**Project Type**: CLI tool
**Performance Goals**: Output formatting is synchronous and CPU-bound on small data; no performance targets beyond "instantaneous for vault sizes < 10 000 secrets"
**Constraints**: Zero regression on existing `table` output (SC-003); no new mandatory dependencies
**Scale/Scope**: Small feature — 1 new module, 1 new command, refactor of 2 existing handlers

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Security by Default | ✓ PASS | `format.rs` only reads secret values already decrypted in memory by the vault layer; no new writes, no new plaintext files |
| II. Determinism | ✓ PASS | Output is deterministic given the same vault contents and `--format` value; JSON key order is stable via struct field order |
| III. Rust Best Practices | ✓ PASS | New code uses `Result<T, E>`, no `unwrap()`, `#[derive(thiserror::Error)]` for `FormatError`; tests required for `format.rs` |
| IV. Modularity (4-layer) | ✓ PASS | `format.rs` lives in the UI/CLI layer; it receives already-resolved `Vec<(String, String)>` data from command handlers — no direct DB or crypto access |
| V. Language | ✓ PASS | All identifiers, docs, and error messages in English |

## Project Structure

### Documentation (this feature)

```text
specs/008-output-formats/
├── plan.md              ← this file
├── research.md          ← Phase 0 output
├── contracts/
│   └── output-formats.md
├── quickstart.md
└── tasks.md             ← /speckit.tasks output (not created here)
```

### Source Code Changes

```text
src/
  cli/
    mod.rs         — add `format: OutputFormat` field to Cli struct; pass to all cmd_* handlers
    commands.rs    — refactor cmd_list, cmd_get to collect data then call format::print_output;
                     add cmd_export
    format.rs      — NEW: OutputFormat enum (ValueEnum), OutputData enum, print_output(), format helpers
    error.rs       — add FormatError variant to CliError (invalid format value already handled by clap)
tests/
  cli_integration.rs  — add --format json / dotenv / shell round-trip tests (ignored in CI, keyring)
  format_unit.rs      — NEW: pure unit tests for format.rs (no keyring, not ignored)
```

## Milestone 1 — Core Formatting Engine (`src/cli/format.rs`)

**Goal**: A standalone module that takes structured data and writes formatted output to any `Write` sink. No CLI wiring yet — fully unit-testable in isolation.

### 1.1 Define `OutputFormat`

```rust
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Table,   // default — preserves existing println! behaviour
    Json,
    Dotenv,
    Shell,
}

impl Default for OutputFormat {
    fn default() -> Self { OutputFormat::Table }
}
```

### 1.2 Define `OutputData`

```rust
pub enum OutputData<'a> {
    SecretList { env: &'a str, secrets: &'a [(String, String)] },
    SecretItem { key: &'a str, value: &'a str },
    ExportList { env: &'a str, secrets: &'a [(String, String)] },
    NotFound   { key: &'a str },
}
```

### 1.3 `print_output` entry point

```rust
pub fn print_output(
    format: OutputFormat,
    data: OutputData<'_>,
    writer: &mut impl std::io::Write,
) -> Result<(), FormatError>
```

Dispatches to private helpers:
- `fmt_table(data, writer)` — calls existing `println!` logic (extracted from `cmd_list`/`cmd_get`)
- `fmt_json(data, writer)` — serialises via `serde_json`
- `fmt_dotenv(data, writer)` — `KEY=value\n` per pair
- `fmt_shell(data, writer)` — `export KEY='value'\n` with single-quote escaping

### 1.4 Shell escaping

```rust
fn shell_escape(value: &str) -> String {
    value.replace('\'', r"'\''")
}
// Wrap: format!("export {}='{}'", key, shell_escape(value))
```

### 1.5 JSON output structs (private to `format.rs`)

```rust
#[derive(serde::Serialize)]
struct SecretPair<'a> { key: &'a str, value: &'a str }

#[derive(serde::Serialize)]
struct ListJson<'a> { secrets: Vec<SecretPair<'a>> }

#[derive(serde::Serialize)]
struct ItemJson<'a> { key: &'a str, value: &'a str }

#[derive(serde::Serialize)]
struct ExportJson<'a> { environment: &'a str, secrets: Vec<SecretPair<'a>> }

#[derive(serde::Serialize)]
struct ErrorJson<'a> { error: &'a str }
```

### 1.6 Unit tests (`tests/format_unit.rs`)

- `table_list_empty` — `OutputData::SecretList { secrets: &[] }` → `"(no secrets in development)\n"` on stderr path
- `json_list` — valid JSON with expected keys
- `json_not_found` — `{"error":"key not found"}`
- `dotenv_basic` — `KEY=value\n` per pair
- `dotenv_value_with_equals` — value containing `=` sign is passed through unchanged
- `shell_basic` — `export KEY='value'\n`
- `shell_single_quote_escape` — `it's here` → `export K='it'\''s here'`
- `shell_special_chars` — `$VAR`, backticks, `"` — no escaping needed (single-quote context)

---

## Milestone 2 — CLI Wiring (`src/cli/mod.rs` + `commands.rs`)

**Goal**: Hook `format.rs` into the argument parser and refactor existing read commands.

### 2.1 Add `--format` to `Cli`

```rust
#[derive(Parser)]
#[command(name = "envy", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format (default: table)
    #[arg(long, short = 'f', global = true, default_value = "table")]
    pub format: OutputFormat,
}
```

`global = true` propagates the flag to all subcommands. `default_value = "table"` means omitting `--format` produces `OutputFormat::Table` — preserving existing behaviour (FR-011, SC-003).

### 2.2 Add `Export` subcommand

```rust
/// Print all secrets for an environment to stdout (default format: dotenv)
Export {
    /// Environment name (e.g. development, production)
    #[arg(short = 'e', long = "env", value_name = "ENV", default_value = "development")]
    env: String,
},
```

> **QA update (2026-03-25)**: Changed from a positional argument to a named `-e/--env` flag to match the rest of the CLI.

The `export` command's default format must be `dotenv`, not `table`. Handle this in `cmd_export`:

```rust
let effective_format = if matches!(format, OutputFormat::Table) {
    OutputFormat::Dotenv   // export defaults to dotenv, not table
} else {
    format
};
```

### 2.3 Refactor `cmd_list`

Before (current):
```rust
pub fn cmd_list(vault: &Vault, env: &str) -> Result<(), CliError> {
    let keys = vault.list_secrets(env)?;
    if keys.is_empty() {
        eprintln!("(no secrets in {env})");
    } else {
        for k in &keys {
            println!("{k}");
        }
    }
    Ok(())
}
```

After:
```rust
pub fn cmd_list(vault: &Vault, env: &str, format: OutputFormat) -> Result<(), CliError> {
    let secrets = vault.list_secrets_with_values(env)?;  // returns Vec<(String,String)>
    print_output(format, OutputData::SecretList { env, secrets: &secrets }, &mut stdout())?;
    Ok(())
}
```

> **Note**: `list` currently only shows keys, not values. For `table` format, `fmt_table` replicates the current keys-only behaviour. For `json`/`dotenv`/`shell`, values are needed — requires a new vault method `list_secrets_with_values`. For `table`, the existing `list_secrets` (keys only) is sufficient; `fmt_table` can receive an empty-value slice or a keys-only variant.

**Revised approach to avoid a vault API change for `table`**: `OutputData::SecretList` holds `Vec<(String, String)>` where value is `""` for table format (value never rendered). `cmd_list` calls `vault.list_secrets_with_values()` always; the method is added to the vault.

### 2.4 Refactor `cmd_get`

```rust
pub fn cmd_get(vault: &Vault, key: &str, env: &str, format: OutputFormat) -> Result<(), CliError> {
    match vault.get_secret(key, env)? {
        Some(value) => {
            print_output(format, OutputData::SecretItem { key, value: &value }, &mut stdout())?;
        }
        None => {
            print_output(format, OutputData::NotFound { key }, &mut stdout())?;
            return Err(CliError::NotFound(key.to_owned()));
        }
    }
    Ok(())
}
```

### 2.5 Add `cmd_export`

```rust
pub fn cmd_export(vault: &Vault, env: &str, format: OutputFormat) -> Result<(), CliError> {
    let effective = if format == OutputFormat::Table { OutputFormat::Dotenv } else { format };
    let secrets = vault.list_secrets_with_values(env)?;
    print_output(effective, OutputData::ExportList { env, secrets: &secrets }, &mut stdout())?;
    Ok(())
}
```

### 2.6 Vault API addition

Add to the database layer (`src/db/` or wherever `list_secrets` lives):

```rust
pub fn list_secrets_with_values(&self, env: &str) -> Result<Vec<(String, String)>, DbError>
```

Returns key-value pairs in alphabetical order by key (consistent with existing `list_secrets` sort). This is a read-only change to the DB layer with no security implications.

### 2.7 `run()` dispatch update

Pass `cli.format` to all handlers:
```rust
Commands::List { env }    => cmd_list(&vault, &env, cli.format),
Commands::Get { key, env} => cmd_get(&vault, &key, &env, cli.format),
Commands::Export { env }  => cmd_export(&vault, &env, cli.format),
// All other commands: format is parsed but not forwarded (ignored)
```

---

## Milestone 3 — Contracts, Quickstart & Integration Tests

### 3.1 CLI output contract (`contracts/output-formats.md`)

Documents the stable JSON shapes consumers can rely on.

### 3.2 Quickstart (`quickstart.md`)

Five integration scenarios covering each format and the `export` command.

### 3.3 Integration tests (`tests/cli_integration.rs`)

New `#[ignore]` tests (require keyring):
- `list_json_format` — set 2 secrets, `envy list --format json`, parse JSON, assert both present
- `get_json_found` — `envy get KEY --format json`, assert `{"key":..,"value":..}`
- `get_json_not_found` — assert `{"error":"key not found"}` and exit ≠ 0
- `export_dotenv_default` — `envy export` (no `--format`), assert `KEY=value` lines
- `export_shell_format` — assert `export KEY='value'` lines
- `export_dotenv_special_chars` — value with `=` sign, newline handling
- `invalid_format` — `envy list --format xml` → exit 2

---

## Complexity Tracking

No constitution violations. The `src/cli/format.rs` module is squarely in the UI/CLI layer and touches neither the database nor cryptography layers.
