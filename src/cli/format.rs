//! Presentation layer — formats output data according to the selected `--format` flag.
//!
//! This module is the single place where output formatting logic lives.
//! Command handlers collect data into [`OutputData`] and call [`print_output`];
//! they never format or print directly. Adding a new format variant requires
//! changes only to this file (FR-010).
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT access the database or cryptographic primitives.
//! - Receives already-decrypted `(key, value)` pairs from command handlers.

use std::io::Write;

// ---------------------------------------------------------------------------
// OutputFormat — the --format flag value
// ---------------------------------------------------------------------------

/// The output format selected by the global `--format` / `-f` flag.
///
/// `Table` is the default for all commands except `export` (which defaults to
/// `Dotenv`). The coercion from `Table` → `Dotenv` for `export` is handled in
/// [`crate::cli::commands::cmd_export`], not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text (default). Preserves existing output exactly.
    #[default]
    Table,
    /// Valid JSON string mapping keys to values.
    Json,
    /// `KEY=value` one per line — compatible with `.env` files and Docker Compose.
    Dotenv,
    /// `export KEY='value'` one per line — safe to `eval` in POSIX shells.
    Shell,
}

// ---------------------------------------------------------------------------
// OutputData — the payload to render
// ---------------------------------------------------------------------------

/// The structured data that a command passes to [`print_output`].
///
/// Each variant corresponds to a distinct rendering context. Lifetimes avoid
/// cloning the already-decrypted strings unnecessarily.
pub enum OutputData<'a> {
    /// Output for `envy list`: all key-value pairs in the environment.
    /// For `Table` format, only keys are shown (values are `""`).
    SecretList {
        env: &'a str,
        secrets: &'a [(String, String)],
    },
    /// Output for `envy get KEY`: a single found key-value pair.
    SecretItem { key: &'a str, value: &'a str },
    /// Output for `envy export ENV`: all key-value pairs (values always shown).
    ExportList {
        env: &'a str,
        secrets: &'a [(String, String)],
    },
    /// Output for `envy get KEY` when the key does not exist.
    NotFound { key: &'a str },
}

// ---------------------------------------------------------------------------
// Private serde structs for JSON output
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct SecretPair<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(serde::Serialize)]
struct ListJson<'a> {
    secrets: Vec<SecretPair<'a>>,
}

#[derive(serde::Serialize)]
struct ItemJson<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(serde::Serialize)]
struct ExportJson<'a> {
    environment: &'a str,
    secrets: Vec<SecretPair<'a>>,
}

#[derive(serde::Serialize)]
struct ErrorJson<'a> {
    error: &'a str,
}

// ---------------------------------------------------------------------------
// FormatError
// ---------------------------------------------------------------------------

/// Errors that can occur during output formatting.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("write error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json serialisation error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Write `data` to `writer` according to `format`.
///
/// The caller is responsible for passing the correct `OutputData` variant for
/// the command being executed. The writer is typically `&mut std::io::stdout()`
/// but can be any `Write` sink (e.g. a `Vec<u8>` in unit tests).
pub fn print_output(
    format: OutputFormat,
    data: OutputData<'_>,
    writer: &mut impl Write,
) -> Result<(), FormatError> {
    match format {
        OutputFormat::Table => fmt_table(&data, writer),
        OutputFormat::Json => fmt_json(&data, writer),
        OutputFormat::Dotenv => fmt_dotenv(&data, writer),
        OutputFormat::Shell => fmt_shell(&data, writer),
    }
}

// ---------------------------------------------------------------------------
// Private format helpers
// ---------------------------------------------------------------------------

/// Table format — replicates existing stdout behaviour exactly (FR-011, SC-003).
///
/// For `SecretList` / `ExportList`: prints one key per line.
/// For `SecretItem`: prints the raw value with a trailing newline.
/// For `NotFound`: writes nothing (caller decides whether to print to stderr).
fn fmt_table(data: &OutputData<'_>, writer: &mut impl Write) -> Result<(), FormatError> {
    match data {
        OutputData::SecretList { env, secrets } => {
            if secrets.is_empty() {
                // Mirror the existing eprintln! path: write to stderr, nothing on stdout.
                // The caller (cmd_list) handles the eprintln! for the Table case.
                let _ = env; // suppress unused-variable warning; env is for reference only
            } else {
                for (key, _value) in *secrets {
                    writeln!(writer, "{key}")?;
                }
            }
        }
        OutputData::SecretItem { key: _, value } => {
            writeln!(writer, "{value}")?;
        }
        OutputData::ExportList { env: _, secrets } => {
            for (key, value) in *secrets {
                writeln!(writer, "{key}={value}")?;
            }
        }
        OutputData::NotFound { key: _ } => {
            // Table format: nothing on stdout for not-found (exit code signals absence).
        }
    }
    Ok(())
}

/// JSON format — serialises via `serde_json`.
fn fmt_json(data: &OutputData<'_>, writer: &mut impl Write) -> Result<(), FormatError> {
    match data {
        OutputData::SecretList { env: _, secrets } => {
            let payload = ListJson {
                secrets: secrets
                    .iter()
                    .map(|(k, v)| SecretPair { key: k, value: v })
                    .collect(),
            };
            serde_json::to_writer(writer.by_ref(), &payload)?;
            writeln!(writer)?;
        }
        OutputData::SecretItem { key, value } => {
            let payload = ItemJson { key, value };
            serde_json::to_writer(writer.by_ref(), &payload)?;
            writeln!(writer)?;
        }
        OutputData::ExportList { env, secrets } => {
            let payload = ExportJson {
                environment: env,
                secrets: secrets
                    .iter()
                    .map(|(k, v)| SecretPair { key: k, value: v })
                    .collect(),
            };
            serde_json::to_writer(writer.by_ref(), &payload)?;
            writeln!(writer)?;
        }
        OutputData::NotFound { key: _ } => {
            let payload = ErrorJson {
                error: "key not found",
            };
            serde_json::to_writer(writer.by_ref(), &payload)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

/// Dotenv format — `KEY=value\n` per pair.
fn fmt_dotenv(data: &OutputData<'_>, writer: &mut impl Write) -> Result<(), FormatError> {
    let secrets: &[(String, String)] = match data {
        OutputData::SecretList { secrets, .. } => secrets,
        OutputData::ExportList { secrets, .. } => secrets,
        OutputData::SecretItem { key, value } => {
            writeln!(writer, "{key}={value}")?;
            return Ok(());
        }
        OutputData::NotFound { key: _ } => return Ok(()),
    };
    for (key, value) in secrets {
        writeln!(writer, "{key}={value}")?;
    }
    Ok(())
}

/// Escapes a secret value for inclusion in a POSIX single-quoted string.
///
/// Replaces each `'` with `'\''` (end-quote, escaped-quote, re-open-quote),
/// which is the standard POSIX technique safe across all POSIX shells.
///
/// Example: `it's "here"` → `it'\''s "here"` (then wrapped: `'it'\''s "here"'`)
fn shell_escape(value: &str) -> String {
    value.replace('\'', r"'\''")
}

/// Shell format — `export KEY='<escaped-value>'\n` per pair.
fn fmt_shell(data: &OutputData<'_>, writer: &mut impl Write) -> Result<(), FormatError> {
    let secrets: &[(String, String)] = match data {
        OutputData::SecretList { secrets, .. } => secrets,
        OutputData::ExportList { secrets, .. } => secrets,
        OutputData::SecretItem { key, value } => {
            writeln!(writer, "export {key}='{}'", shell_escape(value))?;
            return Ok(());
        }
        OutputData::NotFound { key: _ } => return Ok(()),
    };
    for (key, value) in secrets {
        writeln!(writer, "export {key}='{}'", shell_escape(value))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn render(format: OutputFormat, data: OutputData<'_>) -> String {
        let mut buf = Vec::new();
        print_output(format, data, &mut buf).expect("print_output must not fail");
        String::from_utf8(buf).expect("output must be valid UTF-8")
    }

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // --- Table ---

    #[test]
    fn table_list_keys_only() {
        let secrets = pairs(&[("API_KEY", "abc123"), ("DB_HOST", "localhost")]);
        let out = render(
            OutputFormat::Table,
            OutputData::SecretList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "API_KEY\nDB_HOST\n");
    }

    #[test]
    fn table_list_empty_writes_nothing_to_stdout() {
        // Empty vault: fmt_table produces no stdout output; the caller handles eprintln!
        let secrets: Vec<(String, String)> = vec![];
        let out = render(
            OutputFormat::Table,
            OutputData::SecretList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(
            out, "",
            "empty list must produce no stdout output in table mode"
        );
    }

    #[test]
    fn table_item() {
        let out = render(
            OutputFormat::Table,
            OutputData::SecretItem {
                key: "API_KEY",
                value: "abc123",
            },
        );
        assert_eq!(out, "abc123\n");
    }

    // --- JSON ---

    #[test]
    fn json_list_found() {
        let secrets = pairs(&[("API_KEY", "abc123"), ("DB_HOST", "localhost")]);
        let out = render(
            OutputFormat::Json,
            OutputData::SecretList {
                env: "development",
                secrets: &secrets,
            },
        );
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("must be valid JSON");
        let arr = v["secrets"].as_array().expect("secrets must be array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["key"], "API_KEY");
        assert_eq!(arr[0]["value"], "abc123");
    }

    #[test]
    fn json_list_empty() {
        let secrets: Vec<(String, String)> = vec![];
        let out = render(
            OutputFormat::Json,
            OutputData::SecretList {
                env: "development",
                secrets: &secrets,
            },
        );
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("must be valid JSON");
        assert_eq!(v["secrets"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_item_found() {
        let out = render(
            OutputFormat::Json,
            OutputData::SecretItem {
                key: "API_KEY",
                value: "abc123",
            },
        );
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("must be valid JSON");
        assert_eq!(v["key"], "API_KEY");
        assert_eq!(v["value"], "abc123");
    }

    #[test]
    fn json_not_found() {
        let out = render(OutputFormat::Json, OutputData::NotFound { key: "MISSING" });
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("must be valid JSON");
        assert_eq!(v["error"], "key not found");
    }

    #[test]
    fn json_export() {
        let secrets = pairs(&[("DB_PASS", "s3cr3t")]);
        let out = render(
            OutputFormat::Json,
            OutputData::ExportList {
                env: "production",
                secrets: &secrets,
            },
        );
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("must be valid JSON");
        assert_eq!(v["environment"], "production");
        assert_eq!(v["secrets"][0]["key"], "DB_PASS");
        assert_eq!(v["secrets"][0]["value"], "s3cr3t");
    }

    // --- Dotenv ---

    #[test]
    fn dotenv_basic() {
        let secrets = pairs(&[("API_KEY", "abc123"), ("DB_HOST", "localhost")]);
        let out = render(
            OutputFormat::Dotenv,
            OutputData::ExportList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "API_KEY=abc123\nDB_HOST=localhost\n");
    }

    #[test]
    fn dotenv_value_with_equals() {
        // Values containing '=' must pass through unchanged.
        let secrets = pairs(&[("JDBC_URL", "jdbc:mysql://host/db?user=admin&pass=x")]);
        let out = render(
            OutputFormat::Dotenv,
            OutputData::ExportList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "JDBC_URL=jdbc:mysql://host/db?user=admin&pass=x\n");
    }

    // --- Shell ---

    #[test]
    fn shell_basic() {
        let secrets = pairs(&[("DB_PASS", "s3cr3t")]);
        let out = render(
            OutputFormat::Shell,
            OutputData::ExportList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "export DB_PASS='s3cr3t'\n");
    }

    #[test]
    fn shell_single_quote_escape() {
        // POSIX escape: it's here → 'it'\''s here'
        let secrets = pairs(&[("MSG", "it's here")]);
        let out = render(
            OutputFormat::Shell,
            OutputData::ExportList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "export MSG='it'\\''s here'\n");
    }

    #[test]
    fn shell_special_chars_no_extra_escaping() {
        // $VAR, backticks, and double-quotes need no escaping inside single-quotes.
        let secrets = pairs(&[("CMD", "$HOME/`echo foo` and \"quotes\"")]);
        let out = render(
            OutputFormat::Shell,
            OutputData::ExportList {
                env: "development",
                secrets: &secrets,
            },
        );
        assert_eq!(out, "export CMD='$HOME/`echo foo` and \"quotes\"'\n");
    }

    #[test]
    fn shell_escape_unit() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("it's"), "it'\\''s");
        assert_eq!(shell_escape("a'b'c"), "a'\\''b'\\''c");
    }
}
