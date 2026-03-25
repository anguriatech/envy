# Research: Machine-Readable Output Formats

**Feature**: 008-output-formats
**Date**: 2026-03-24

---

## Decision 1: Global `--format` flag placement — `Cli` struct vs per-command

**Decision**: Add `format: OutputFormat` to the `Cli` struct (the top-level parser), not to each individual command.

**Rationale**: `clap` propagates fields added to the root `Cli` struct to all subcommands automatically when using the derive API. This satisfies FR-001 ("global flag applicable to all commands") with a single field rather than duplicating it across 9 subcommand structs. The `run()` function already passes the parsed `Cli` to command handlers, so `format` can be forwarded trivially.

**Alternatives considered**:
- Per-command field: Works but requires duplication and doesn't satisfy FR-001's "global" intent.
- Separate `FormatArgs` flatten struct: Adds indirection with no benefit at this scale.

---

## Decision 2: `OutputFormat` as `clap::ValueEnum`

**Decision**: Derive `clap::ValueEnum` on `OutputFormat` so `clap` handles validation and generates `--help` text automatically.

**Rationale**: `ValueEnum` produces compile-time-checked string↔enum mapping, automatic `--format <table|json|dotenv|shell>` in help, and an `Err` from `clap` (exit 2) for invalid values — satisfying FR-012 without any manual validation code.

**Alternatives considered**:
- Manual `String` argument with `match`: Requires custom validation, duplicates accepted values in help text and code.

---

## Decision 3: Presentation layer — `src/cli/format.rs` module with free functions

**Decision**: Create `src/cli/format.rs` with a single public entry point `print_output(format: OutputFormat, data: &OutputData, writer: &mut impl Write)` where `OutputData` is an enum covering the three printable payloads (`SecretList`, `SecretItem`, `ExportList`).

**Rationale**: Satisfies FR-010 (adding a new format touches only `format.rs`) without over-engineering. A trait (`Formattable`) would be cleaner if the number of output types were large and varied, but with three concrete types a simple enum dispatch is more readable and requires less boilerplate. The `writer: &mut impl Write` parameter makes the function unit-testable without capturing stdout.

**Alternatives considered**:
- `Formattable` trait on each output struct: More extensible, but adds a trait bound and `dyn`/`impl` overhead with no current benefit.
- Inline formatting in each `cmd_*` handler: Violates FR-010 (adding a format requires touching every handler).

---

## Decision 4: Shell single-quote escaping — POSIX `'...'` with `'\''` replacement

**Decision**: Escape values for `shell` format by:
1. Wrapping the entire value in single quotes.
2. Replacing each embedded `'` with `'\''` (end quote, escaped quote, start quote).

```
value:  it's "here" and $SPECIAL
output: export KEY='it'\''s "here" and $SPECIAL'
```

**Rationale**: This is the POSIX-standard technique. It is safe under all POSIX shells (bash, sh, zsh, dash) and does not require knowledge of the shell being used. Double-quote escaping would require escaping `$`, `` ` ``, `\`, `!`, and `"`, which is more error-prone.

**Implementation**: `value.replace("'", r"'\''")` in Rust, then wrap: `format!("export {}='{}'", key, escaped)`.

**Alternatives considered**:
- `$'...'` ANSI-C quoting: Not portable (not available in plain `sh`).
- Double-quote escaping: Requires escaping more characters and is shell-dependent for `!`.

---

## Decision 5: `serde_json` output for `json` format

**Decision**: Define lightweight output structs (`SecretListOutput`, `SecretItemOutput`) with `#[derive(Serialize)]` in `format.rs` and use `serde_json::to_string` for serialisation.

**Rationale**: `serde_json` is already a dependency (used for `envy.enc`). Dedicated output structs (rather than re-using internal types) decouple the public JSON schema from the internal data model — a change to an internal type does not break the stable output contract.

**JSON shapes**:
```json
// envy list --format json
{"secrets": [{"key": "API_KEY", "value": "abc123"}, ...]}

// envy get KEY --format json (found)
{"key": "API_KEY", "value": "abc123"}

// envy get KEY --format json (not found)
{"error": "key not found"}

// envy export ENV --format json
{"environment": "production", "secrets": [{"key": "DB_PASS", "value": "secret"}]}
```

**Alternatives considered**:
- Flat `{"API_KEY": "abc123"}` for list/export: Simpler but harder to extend with metadata (environment name, timestamps) without a breaking change.

---

## Decision 6: `export` command — reads local vault, not `envy.enc`

**Decision**: `envy export ENV` queries the local SQLite vault directly (same as `list` + `get` but for all keys at once). It does NOT decrypt `envy.enc`.

**Rationale**: Spec explicitly scopes `export` to the local vault (Assumptions section). Decrypting `envy.enc` would require passphrase input, complicating the headless/pipeline use case. The two operations (`decrypt` then `export`) can be composed on the command line.

---

## Decision 7: `--format` on write commands — accepted, ignored

**Decision**: The flag is parsed globally and reaches all handlers, but `cmd_set`, `cmd_rm`, `cmd_init`, `cmd_migrate`, `cmd_encrypt`, `cmd_decrypt`, `cmd_run` simply ignore the `format` parameter.

**Rationale**: Silently accepting an unused flag is better than a surprising "unknown flag" error in scripted pipelines that pass `--format json` globally before a write command. This matches the spec Assumption: "`--format` on write commands is accepted but has no effect on output content."

---

## Decision 8: `table` format unchanged — no ANSI codes when stdout is not a TTY

**Decision**: The existing `table` output path (current `println!` calls in `cmd_list` and `cmd_get`) is preserved unchanged for the `table` format. ANSI colour suppression for non-TTY stdout is **not** introduced in this feature (it is already handled by the existing output, which uses no colour codes in `cmd_list`/`cmd_get`).

**Rationale**: SC-003 requires byte-for-byte identical output when `--format` is omitted. Touching the existing output path risks a regression. ANSI suppression is a separate concern scoped out by the spec.
