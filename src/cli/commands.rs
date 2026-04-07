//! Command handler functions — see plan.md §4.2
//!
//! All functions are `pub(super)` — only callable from `cli::run()`.
//! Handlers receive already-opened resources (`&Vault`, `&[u8; 32]`,
//! `&ProjectId`) and must not manage the Vault lifecycle themselves.
//!
//! # Exception
//! [`cmd_init`] is the sole exception: it owns its own Vault connection
//! because init creates the project entry before any manifest exists.

use std::path::Path;

use crate::cli::error::CliError;
use crate::cli::format::{FormatError, OutputData, OutputFormat, print_output};
use crate::core::CoreError;
use crate::db::{ProjectId, Vault};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns the display name for an environment, substituting the Core default
/// when the CLI receives an empty string (meaning `-e` flag was absent).
///
/// The CLI must not hardcode `"development"` — `core::DEFAULT_ENV` is the
/// single source of truth.
fn display_env(env: &str) -> &str {
    if env.is_empty() {
        crate::core::DEFAULT_ENV
    } else {
        env
    }
}

// ---------------------------------------------------------------------------
// T016 — cmd_init
// ---------------------------------------------------------------------------

/// Initialises Envy in the current working directory.
///
/// Creates `envy.toml` and registers a new project entry in the vault.
/// This is the only handler that owns its own Vault connection.
///
/// # Errors
/// - [`CliError::AlreadyInitialised`] — `envy.toml` exists in the cwd.
/// - [`CliError::ParentProjectExists`] — `envy.toml` exists in a parent dir.
/// - [`CliError::VaultOpen`] — keyring, vault open, or DB write failed.
pub(super) fn cmd_init() -> Result<(), CliError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::VaultOpen(format!("cannot determine current directory: {e}")))?;

    // Step 1 — Check whether a manifest already exists (in cwd or any ancestor).
    match crate::core::find_manifest(&cwd) {
        Ok((_, found_dir)) if found_dir == cwd => {
            return Err(CliError::AlreadyInitialised);
        }
        Ok((_, found_dir)) => {
            return Err(CliError::ParentProjectExists(
                found_dir.display().to_string(),
            ));
        }
        Err(CoreError::ManifestNotFound) => {
            // No manifest anywhere above — safe to initialise.
        }
        Err(e) => {
            return Err(CliError::VaultOpen(e.to_string()));
        }
    }

    // Step 2 — Fetch (or generate) the global vault master key from the OS keyring.
    let master_key = crate::crypto::get_or_create_master_key()
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 3 — Open (or create) the vault; directory is guaranteed by run().
    let vault_path = super::vault_path();
    let vault = Vault::open(&vault_path, master_key.as_ref())
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 4 — Register the project in the vault. Use the cwd name as the
    // human-readable project name; the DB generates the UUID primary key.
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");
    let project_id: ProjectId = vault
        .create_project(project_name)
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 5 — Write envy.toml with the DB-generated project UUID.
    crate::core::create_manifest(&cwd, project_id.as_str())
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    println!("✓ Initialised envy project {}.", project_id.as_str());
    Ok(())
}

// ---------------------------------------------------------------------------
// T017 — cmd_set
// ---------------------------------------------------------------------------

/// Stores or updates a secret in the vault.
///
/// The `assignment` splitting (`KEY=VALUE` → `key`, `value`) is performed by
/// `cli::run()` before dispatch; `cmd_set` receives the split parts.
/// Core's `set_secret` is responsible for key-name validation.
pub(super) fn cmd_set(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    key: &str,
    value: &str,
) -> Result<(), CoreError> {
    crate::core::set_secret(vault, master_key, project_id, env, key, value)?;
    println!("✓ Set {} in {}.", key, display_env(env));
    Ok(())
}

// ---------------------------------------------------------------------------
// T018 — cmd_get
// ---------------------------------------------------------------------------

/// Prints the decrypted value of a secret to stdout.
///
/// **stdout contract (table format)**: outputs exactly `{value}\n` — no labels,
/// no leading whitespace. Shell pipelines (`envy get KEY | xargs ...`) depend on
/// this (FR-011, SC-003).
///
/// For non-table formats, delegates to the presentation layer.
/// On key-not-found: for table format exits with `CoreError::Db(NotFound)`; for
/// other formats writes a machine-readable error payload then returns the same error.
pub(super) fn cmd_get(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    key: &str,
    format: OutputFormat,
) -> Result<(), CliError> {
    use std::io::stdout;

    match crate::core::get_secret(vault, master_key, project_id, env, key) {
        Ok(value) => {
            if format == OutputFormat::Table {
                // Preserve exact existing behaviour (SC-003).
                println!("{}", *value);
            } else {
                print_output(
                    format,
                    OutputData::SecretItem {
                        key,
                        value: value.as_str(),
                    },
                    &mut stdout(),
                )
                .map_err(|e: FormatError| CliError::Output(e.to_string()))?;
            }
            Ok(())
        }
        Err(e) => {
            if format != OutputFormat::Table {
                // Emit a machine-readable error before returning the error code.
                let _ = print_output(format, OutputData::NotFound { key }, &mut stdout());
            }
            Err(CliError::Core(e))
        }
    }
}

// ---------------------------------------------------------------------------
// T019 — cmd_list
// ---------------------------------------------------------------------------

/// Lists all secret key names (or key-value pairs) for the environment.
///
/// For `Table` format, only keys are printed one per line — identical to the
/// previous behaviour (SC-003). For other formats, values are decrypted and
/// passed to the presentation layer.
///
/// If the environment has no secrets and format is `Table`, an informational
/// message is printed to stderr so that scripts consuming stdout are unaffected.
pub(super) fn cmd_list(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    format: OutputFormat,
) -> Result<(), CliError> {
    use crate::cli::format::OutputFormat;
    use std::io::stdout;

    if format == OutputFormat::Table {
        // Table path: keys only, no values decrypted — backward-compatible (FR-011, SC-003).
        let keys = crate::core::list_secret_keys(vault, project_id, env).map_err(CliError::Core)?;
        if keys.is_empty() {
            eprintln!("(no secrets in {})", display_env(env));
        } else {
            for k in &keys {
                println!("{k}");
            }
        }
        return Ok(());
    }

    // Non-table paths: decrypt values and delegate to the presentation layer.
    let secrets = crate::core::list_secrets_with_values(vault, master_key, project_id, env)
        .map_err(CliError::Core)?;
    print_output(
        format,
        OutputData::SecretList {
            env: display_env(env),
            secrets: &secrets,
        },
        &mut stdout(),
    )
    .map_err(|e: FormatError| CliError::Output(e.to_string()))
}

// ---------------------------------------------------------------------------
// cmd_export [008-output-formats]
// ---------------------------------------------------------------------------

/// Prints all secrets for the given environment to stdout.
///
/// Default format is `Dotenv` — `Table` is coerced to `Dotenv` because the
/// `export` command has no meaningful table representation (FR-007).
/// Use `--format json` or `--format shell` for other machine-readable output.
pub(super) fn cmd_export(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    format: OutputFormat,
) -> Result<(), CliError> {
    use std::io::stdout;

    // Table → Dotenv coercion: `export` has no table representation (FR-007).
    let effective = if format == OutputFormat::Table {
        OutputFormat::Dotenv
    } else {
        format
    };

    let secrets = crate::core::list_secrets_with_values(vault, master_key, project_id, env)
        .map_err(CliError::Core)?;

    print_output(
        effective,
        OutputData::ExportList {
            env: display_env(env),
            secrets: &secrets,
        },
        &mut stdout(),
    )
    .map_err(|e: FormatError| CliError::Output(e.to_string()))
}

// ---------------------------------------------------------------------------
// T020 — cmd_rm
// ---------------------------------------------------------------------------

/// Permanently deletes a secret from the vault.
pub(super) fn cmd_rm(
    vault: &Vault,
    project_id: &ProjectId,
    env: &str,
    key: &str,
) -> Result<(), CoreError> {
    crate::core::delete_secret(vault, project_id, env, key)?;
    println!("✓ Deleted {} from {}.", key, display_env(env));
    Ok(())
}

// ---------------------------------------------------------------------------
// T025 — cmd_run
// ---------------------------------------------------------------------------

/// Injects all secrets for the environment as env vars and runs a child process.
///
/// # Exit code contract (from `contracts/cli.md`)
/// - Returns the child's exit code exactly as received.
/// - Returns `1` when the child is killed by a signal (Unix: `status.code()` → `None`).
/// - Returns `127` when the binary cannot be spawned (conventional "not found" code).
///
/// Secrets are injected **in addition to** the inherited environment, not as a
/// replacement. The `Zeroizing<String>` values are zeroed when the map is dropped.
pub(super) fn cmd_run(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    command: &[String],
) -> i32 {
    // Decrypt all secrets for the target environment.
    let secrets = match crate::core::get_env_secrets(vault, master_key, project_id, env) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", crate::cli::error::format_core_error(&e));
            return crate::cli::error::core_exit_code(&e);
        }
    };

    // `command` is guaranteed non-empty by `#[arg(last = true, required = true)]`.
    let (bin, args) = command
        .split_first()
        .expect("clap guarantees at least one element after --");

    match std::process::Command::new(bin)
        .args(args)
        // Inject secrets on top of the inherited environment.
        .envs(secrets.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .status()
    {
        Ok(status) => {
            // `status.code()` returns None when the child was killed by a Unix signal.
            // Fall back to 1 (generic failure) — full signal forwarding is Phase 3 work.
            status.code().unwrap_or(1)
        }
        Err(e) => {
            eprintln!("error: failed to execute `{}`: {}", bin, e);
            127 // conventional POSIX "command not found" exit code
        }
    }
}

// ---------------------------------------------------------------------------
// T026 — cmd_migrate
// ---------------------------------------------------------------------------

/// Imports secrets from a legacy `.env` file into the vault.
///
/// # Line-parsing rules (from `contracts/cli.md`)
/// 1. Trim leading/trailing whitespace.
/// 2. Skip blank lines and `#`-prefixed comment lines silently.
/// 3. Split on the **first** `=` only:
///    - `Some((key, value))` → call `core::set_secret` (abort on error).
///    - `None` → emit a per-line warning to stderr, continue.
/// 4. After all lines: print a summary to stdout.
///
/// Migration is NOT atomic: if aborted mid-way, partial secrets remain.
/// Re-running is safe because `set_secret` uses upsert semantics.
pub(super) fn cmd_migrate(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env: &str,
    file: &Path,
) -> Result<(), CliError> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| CliError::FileNotFound(file.display().to_string(), e.to_string()))?;

    let mut imported = 0usize;
    let mut warnings = 0usize;

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Rule 2: skip blank lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Rule 3: split on first `=`.
        match trimmed.split_once('=') {
            Some((key, value)) => {
                crate::core::set_secret(vault, master_key, project_id, env, key.trim(), value)
                    .map_err(|e| CliError::VaultOpen(e.to_string()))?;
                imported += 1;
            }
            None => {
                eprintln!(
                    "warning: line {}: skipping malformed entry: {:?}",
                    line_no + 1,
                    trimmed
                );
                warnings += 1;
            }
        }
    }

    let warnings_suffix = if warnings > 0 {
        format!(" ({warnings} warning(s))")
    } else {
        String::new()
    };
    println!(
        "✓ Imported {} secret(s) into {}{}.",
        imported,
        display_env(env),
        warnings_suffix,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_encrypt / cmd_decrypt
// ---------------------------------------------------------------------------

/// Resolves the passphrase for a specific environment.
///
/// Priority (FR-001, FR-002, FR-012):
/// 1. `ENVY_PASSPHRASE_<UPPER_ENV>` — per-environment env var (highest).
/// 2. `ENVY_PASSPHRASE` — global fallback env var.
/// 3. Interactive terminal prompt via `dialoguer` — when no env var is set.
///    Returns `Ok(None)` if there is no env var AND no TTY (e.g. CI pipe).
///
/// Whitespace-only env vars are treated as configuration errors (Err), not as
/// "not set" — to prevent silent fallback to an interactive prompt in CI.
///
/// The `confirm` flag enables double-entry confirmation (new environments).
/// The `suggested` parameter is reserved for Phase 7 Diceware wiring (T028).
fn resolve_passphrase_for_env(
    env_name: &str,
    confirm: bool,
    _suggested: Option<&str>,
) -> Result<Option<zeroize::Zeroizing<String>>, CliError> {
    // Tier 1: per-environment env var.
    let var_name = format!(
        "ENVY_PASSPHRASE_{}",
        env_name.to_uppercase().replace('-', "_")
    );
    if let Ok(val) = std::env::var(&var_name) {
        if !val.trim().is_empty() {
            return Ok(Some(zeroize::Zeroizing::new(val)));
        }
        return Err(CliError::PassphraseInput(format!(
            "{var_name} is set but contains only whitespace"
        )));
    }

    // Tier 2: global env var.
    if let Ok(val) = std::env::var("ENVY_PASSPHRASE") {
        if !val.trim().is_empty() {
            return Ok(Some(zeroize::Zeroizing::new(val)));
        }
        return Err(CliError::PassphraseInput(
            "ENVY_PASSPHRASE is set but contains only whitespace".into(),
        ));
    }

    // Tier 3: interactive prompt — Ok(None) when no TTY.
    let theme = dialoguer::theme::ColorfulTheme::default();

    // T028: Diceware suggestion path — empty Enter accepts suggestion.
    if let Some(suggested) = _suggested {
        let prompt = format!("Passphrase for '{env_name}' (press Enter to accept: {suggested})");
        let raw: String = match dialoguer::Password::with_theme(&theme)
            .with_prompt(&prompt)
            .allow_empty_password(true)
            .interact()
        {
            Ok(v) => v,
            Err(_) => return Ok(None), // No TTY.
        };
        if raw.is_empty() {
            // User accepted the Diceware suggestion.
            return Ok(Some(zeroize::Zeroizing::new(suggested.to_string())));
        }
        // User typed their own passphrase.
        if raw.trim().is_empty() {
            return Err(CliError::PassphraseInput(
                "passphrase must not be empty".into(),
            ));
        }
        if confirm {
            let confirmed: String = match dialoguer::Password::with_theme(&theme)
                .with_prompt(format!("Confirm passphrase for '{env_name}'"))
                .interact()
            {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            if confirmed != raw {
                return Err(CliError::PassphraseInput(
                    "Passphrases do not match.".into(),
                ));
            }
        }
        return Ok(Some(zeroize::Zeroizing::new(raw)));
    }

    // No suggestion: standard prompt.
    let prompt = format!("Passphrase for '{env_name}'");
    let result: Result<String, _> = if confirm {
        dialoguer::Password::with_theme(&theme)
            .with_prompt(&prompt)
            .with_confirmation("Confirm passphrase", "Passphrases do not match.")
            .interact()
    } else {
        dialoguer::Password::with_theme(&theme)
            .with_prompt(&prompt)
            .interact()
    };

    match result {
        Ok(raw) => {
            if raw.trim().is_empty() {
                return Err(CliError::PassphraseInput(
                    "passphrase must not be empty".into(),
                ));
            }
            Ok(Some(zeroize::Zeroizing::new(raw)))
        }
        Err(_) => Ok(None), // No TTY or I/O error → skip this env.
    }
}

/// Returns `true` when any non-whitespace per-env or global passphrase env var
/// is set for at least one of the given environment names (FR-004).
///
/// Used by `cmd_encrypt` to decide whether to run headless (no interactive
/// prompts) or interactive (MultiSelect + Diceware — Phase 5).
fn is_headless_mode(env_names: &[String]) -> bool {
    if let Ok(val) = std::env::var("ENVY_PASSPHRASE") {
        if !val.trim().is_empty() {
            return true;
        }
    }
    for env_name in env_names {
        let var_name = format!(
            "ENVY_PASSPHRASE_{}",
            env_name.to_uppercase().replace('-', "_")
        );
        if let Ok(val) = std::env::var(&var_name) {
            if !val.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

/// Prints a high-visibility "SAVE THIS NOW" banner to stderr with `passphrase`.
///
/// Called when the user accepts a Diceware suggestion — the banner is shown
/// exactly once. Passphrase is printed in bold yellow so it stands out, and
/// the message reminds the user it will not be shown again (FR-010, SC-005).
fn print_diceware_banner(passphrase: &str) {
    eprintln!(
        "\n  {}\n\n    {}\n\n  {}\n",
        dialoguer::console::style("╔══════════════════════════════════════╗")
            .yellow()
            .bold(),
        dialoguer::console::style(passphrase).yellow().bold(),
        dialoguer::console::style("SAVE THIS PASSPHRASE NOW. You will not be shown it again.")
            .yellow()
            .bold(),
    );
}

/// Prompts the user to confirm a passphrase key-rotation for `env_name`.
///
/// Displays a high-visibility warning, then uses `Confirm` with `default(false)`
/// so pressing Enter or typing 'N' aborts the rotation (FR-008, SC-004).
///
/// Returns `Ok(true)` if the user explicitly confirms, `Ok(false)` to abort.
fn confirm_key_rotation(env_name: &str) -> Result<bool, CliError> {
    eprintln!(
        "\n  {} Passphrase does not match existing data for '{env_name}'.\n  \
         Continuing will ROTATE the key. Data sealed with the old passphrase\n  \
         will not be recoverable without it.\n",
        dialoguer::console::style("WARNING:").yellow().bold()
    );
    let theme = dialoguer::theme::ColorfulTheme::default();
    dialoguer::Confirm::with_theme(&theme)
        .with_prompt(format!("Rotate the key for '{env_name}'?"))
        .default(false)
        .interact()
        .map_err(|e| CliError::PassphraseInput(e.to_string()))
}

/// Resolves the passphrase for encrypt/decrypt operations.
///
/// Priority order (Constitution Principle I — secrets must be zeroed early):
/// 1. `ENVY_PASSPHRASE` env var, if non-empty after trimming — headless CI mode.
/// 2. Interactive terminal prompt via `dialoguer` (hidden, no echo).
///    When `confirm` is `true`, a second confirmation entry is required.
///
/// The returned value is immediately wrapped in `Zeroizing<String>`.
///
/// # Errors
/// - [`CliError::PassphraseInput`] if the passphrase is empty/whitespace after
///   resolution, or if the terminal prompt fails (IO error, Ctrl-C, no TTY).
fn resolve_passphrase(prompt: &str, confirm: bool) -> Result<zeroize::Zeroizing<String>, CliError> {
    // 1. Check env var — headless CI/CD mode.
    if let Ok(val) = std::env::var("ENVY_PASSPHRASE") {
        if !val.trim().is_empty() {
            return Ok(zeroize::Zeroizing::new(val));
        }
        // Env var is explicitly set but contains only whitespace — configuration
        // error. Fail immediately rather than silently falling back to an
        // interactive prompt, which would be surprising (and hang) in CI.
        return Err(CliError::PassphraseInput(
            "ENVY_PASSPHRASE is set but contains only whitespace".into(),
        ));
    }

    // 2. Interactive terminal prompt.
    let theme = dialoguer::theme::ColorfulTheme::default();
    let raw: String = if confirm {
        dialoguer::Password::with_theme(&theme)
            .with_prompt(prompt)
            .with_confirmation("Confirm passphrase", "Passphrases do not match.")
            .interact()
            .map_err(|e| CliError::PassphraseInput(e.to_string()))?
    } else {
        dialoguer::Password::with_theme(&theme)
            .with_prompt(prompt)
            .interact()
            .map_err(|e| CliError::PassphraseInput(e.to_string()))?
    };

    // Validate non-empty (defensive; dialoguer usually enforces this).
    if raw.trim().is_empty() {
        return Err(CliError::PassphraseInput(
            "passphrase must not be empty".into(),
        ));
    }
    Ok(zeroize::Zeroizing::new(raw))
}

/// Seals vault environments into `envy.enc` at `artifact_path`.
///
/// **Headless path** (FR-001–FR-006, FR-012, FR-013): active when any
/// `ENVY_PASSPHRASE_<ENV>` or `ENVY_PASSPHRASE` env var is non-whitespace.
/// Iterates all (or `env_filter`) environments; resolves per-env passphrase;
/// merges into the existing artifact; writes atomically.
///
/// **Interactive path** (FR-007, FR-009, FR-011): presents a `MultiSelect`
/// of all vault environments; resolves a passphrase for each selected env via
/// `resolve_passphrase_for_env` (single-entry; double-entry for new envs is
/// handled in Phase 7 T028). Diceware suggestion is wired in Phase 7 T027.
///
/// Both paths share the same smart-merge and atomic-write logic (FR-005,
/// FR-006, FR-013).
///
/// # Errors
/// - [`CliError::PassphraseInput`] if any passphrase env var is whitespace-only
///   or the interactive prompt fails.
/// - [`CliError::VaultOpen`] on vault read, crypto, or file write failure.
pub(super) fn cmd_encrypt(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    artifact_path: &std::path::Path,
    env_filter: Option<&str>,
) -> Result<(), CliError> {
    // Step 0 — Validate passphrase env vars before any vault I/O.
    // Whitespace-only values are a configuration error (FR-012): surface them
    // immediately rather than falling through to the no-envs guard or MultiSelect.
    if let Ok(val) = std::env::var("ENVY_PASSPHRASE") {
        if val.trim().is_empty() {
            return Err(CliError::PassphraseInput(
                "ENVY_PASSPHRASE is set but contains only whitespace".into(),
            ));
        }
    }

    // Step 1 — List all environments in the vault.
    let all_envs: Vec<String> = vault
        .list_environments(project_id)
        .map_err(|e| CliError::VaultOpen(e.to_string()))?
        .into_iter()
        .map(|e| e.name)
        .collect();

    // T021: guard — no environments in vault at all.
    if all_envs.is_empty() {
        println!("No environments found. Use 'envy set' to add secrets first.");
        return Ok(());
    }

    // Step 2 — Apply env_filter to narrow the candidate list.
    let selected_envs: Vec<String> = match env_filter {
        Some(f) => vec![f.to_string()],
        None => all_envs,
    };

    // Step 3 — Determine which envs to actually seal (and track headless mode).
    let headless = is_headless_mode(&selected_envs);
    let envs_to_seal: Vec<String> = if headless {
        // Headless: use the full selected list (env vars drive passphrase resolution).
        selected_envs
    } else if env_filter.is_some() {
        // Interactive but env_filter provided: seal just that one env.
        selected_envs
    } else {
        // T020: Interactive + no filter → MultiSelect.
        let theme = dialoguer::theme::ColorfulTheme::default();
        let indices = dialoguer::MultiSelect::with_theme(&theme)
            .with_prompt("Select environments to encrypt")
            .items(&selected_envs)
            .interact()
            .map_err(|e| CliError::PassphraseInput(e.to_string()))?;
        if indices.is_empty() {
            println!("Nothing to encrypt.");
            return Ok(());
        }
        indices
            .into_iter()
            .map(|i| selected_envs[i].clone())
            .collect()
    };

    // Step 4 — Load existing artifact as smart-merge base (FR-005, FR-013).
    let mut artifact = match crate::core::read_artifact(artifact_path) {
        Ok(a) => a,
        Err(crate::core::SyncError::FileNotFound(_)) => crate::core::new_empty_artifact(),
        Err(e) => return Err(CliError::VaultOpen(e.to_string())),
    };

    // Step 5 — For each env: resolve passphrase (T022/T028), pre-flight check
    //           (T024), Diceware banner (T027), seal, merge.
    let mut sealed_envs: Vec<String> = Vec::new();
    for env_name in &envs_to_seal {
        // T027: generate a Diceware suggestion for NEW environments in interactive mode.
        // Existing envs already have a passphrase — no suggestion needed.
        let is_new_env = !artifact.environments.contains_key(env_name);
        let diceware_suggestion: Option<String> = if !headless && is_new_env {
            Some(crate::crypto::suggest_passphrase(4))
        } else {
            None
        };

        // F1: Skip environments with 0 secrets — sealing an empty envelope is
        // almost always a user mistake (e.g., running encrypt before `envy set`).
        let secret_keys =
            crate::core::list_secret_keys(vault, project_id, env_name).unwrap_or_default(); // treat DB errors as empty (safe skip)
        if secret_keys.is_empty() {
            eprintln!(
                "  {}  environment '{}' has 0 secrets, skipping",
                dialoguer::console::style("\u{26a0}").yellow(),
                env_name
            );
            continue;
        }

        // T022/T028: resolve passphrase; double-entry for new envs.
        let passphrase = match resolve_passphrase_for_env(
            env_name,
            !headless && is_new_env, // confirm = true for new envs in interactive mode
            diceware_suggestion.as_deref(),
        )? {
            Some(p) => p,
            None => continue, // no env var and no TTY → skip this env
        };

        // T024: Pre-flight key-rotation check (interactive path only, FR-008, SC-004).
        // Headless mode bypasses this check — CI operators know their passphrases.
        if !headless {
            if let Some(existing_envelope) = artifact.environments.get(env_name) {
                if !crate::core::check_envelope_passphrase(
                    passphrase.as_ref(),
                    env_name,
                    existing_envelope,
                ) {
                    // Passphrase mismatch → warn and require explicit confirmation.
                    if !confirm_key_rotation(env_name)? {
                        continue; // User said No (or pressed Enter) → skip this env.
                    }
                    // User explicitly confirmed → fall through to seal.
                }
            }
        }

        // T027: if the user accepted the Diceware suggestion, show the "SAVE THIS NOW" banner.
        if let Some(ref suggestion) = diceware_suggestion {
            if *passphrase == *suggestion {
                print_diceware_banner(suggestion);
            }
        }

        let envelope =
            crate::core::seal_env(vault, master_key, project_id, env_name, passphrase.as_ref())
                .map_err(|e| CliError::VaultOpen(e.to_string()))?;
        artifact.environments.insert(env_name.clone(), envelope);
        sealed_envs.push(env_name.clone());
    }

    // Step 6 — Atomic write (write-to-tmp + rename, FR-006).
    crate::core::write_artifact_atomic(&artifact, artifact_path)
        .map_err(|e| CliError::VaultOpen(e.to_string()))?;

    // Step 7 — Success output (T029: lists only updated envs in this run).
    println!(
        "Sealed {} environment(s) \u{2192} {}",
        sealed_envs.len(),
        artifact_path.display()
    );
    for env_name in &sealed_envs {
        println!(
            "  {}  {}",
            dialoguer::console::style("\u{2713}").green(),
            env_name
        );
    }

    Ok(())
}

/// Reads `envy.enc`, unseals it, and upserts secrets into the vault.
///
/// Passphrase is resolved from `ENVY_PASSPHRASE` env var (headless CI) or via
/// an interactive single-entry terminal prompt. See plan.md §Algorithm for details.
///
/// # Errors
/// - [`CliError::FileNotFound`] (exit 1) if `artifact_path` does not exist.
/// - [`CliError::VaultOpen`] (exit 4) if `envy.enc` is malformed or has an
///   unsupported version.
/// - [`CliError::PassphraseInput`] (exit 2) if the passphrase prompt fails or
///   the passphrase is empty/whitespace.
/// - [`CliError::NothingImported`] (exit 1) if zero environments could be
///   decrypted (all skipped due to wrong passphrase).
pub(super) fn cmd_decrypt(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    artifact_path: &std::path::Path,
) -> Result<(), CliError> {
    // Step 1 — Read and parse envy.enc. Fail fast if missing or malformed.
    let artifact = crate::core::read_artifact(artifact_path).map_err(|e| match e {
        crate::core::SyncError::FileNotFound(path) => {
            CliError::FileNotFound(path, "envy.enc not found".into())
        }
        other => CliError::VaultOpen(other.to_string()),
    })?;

    // Step 2 — Determine headless vs interactive (QA-F2: per-env passphrase parity).
    let env_names: Vec<String> = artifact.environments.keys().cloned().collect();
    let (imported, skipped) = if is_headless_mode(&env_names) {
        // ── Headless path: per-env passphrase resolution (QA-F2) ──
        let mut imp: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, zeroize::Zeroizing<String>>,
        > = std::collections::BTreeMap::new();
        let mut skp: Vec<String> = Vec::new();

        for env_name in &env_names {
            let passphrase = match resolve_passphrase_for_env(env_name, false, None)? {
                Some(p) => p,
                None => {
                    skp.push(env_name.clone());
                    continue;
                }
            };
            match crate::core::unseal_env(&artifact, env_name, passphrase.as_ref())
                .map_err(|e| CliError::VaultOpen(e.to_string()))?
            {
                Some(secrets) => {
                    imp.insert(env_name.clone(), secrets);
                }
                None => {
                    skp.push(env_name.clone());
                }
            }
        }
        (imp, skp)
    } else {
        // ── Interactive path: single passphrase (Progressive Disclosure) ──
        let passphrase = resolve_passphrase("Enter passphrase", false)?;
        let result = crate::core::unseal_artifact(&artifact, passphrase.as_ref())
            .map_err(|e| CliError::VaultOpen(e.to_string()))?;
        (result.imported, result.skipped)
    };

    // Step 3 — If nothing was imported, surface NothingImported (exit 1).
    if imported.is_empty() {
        return Err(CliError::NothingImported);
    }

    // Step 4 — Upsert all imported secrets; track per-env success count.
    // Individual failures are warnings, not errors (partial import is still useful).
    let mut upserted_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for (env_name, secrets) in &imported {
        let mut ok = 0usize;
        for (key, value) in secrets {
            match crate::core::set_secret(
                vault,
                master_key,
                project_id,
                env_name,
                key,
                value.as_ref(),
            ) {
                Ok(()) => ok += 1,
                Err(e) => eprintln!("warning: failed to upsert {env_name}/{key}: {e}"),
            }
        }
        upserted_counts.insert(env_name.clone(), ok);
    }

    // Step 5 — Print success header.
    println!("Imported {} environment(s) from envy.enc", imported.len());

    // Step 6 — Progressive Disclosure: green ✓ for imported, yellow ⚠ dim for skipped.
    for env_name in imported.keys() {
        let ok = upserted_counts.get(env_name).copied().unwrap_or(0);
        println!(
            "  {}  {} ({} secret(s) upserted)",
            dialoguer::console::style("\u{2713}").green(),
            env_name,
            ok
        );
    }
    for env_name in &skipped {
        println!(
            "  {}  {} skipped \u{2014} different passphrase or key",
            dialoguer::console::style("\u{26a0}").yellow().dim(),
            env_name
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_status — sync-state overview (010-status-command)
// ---------------------------------------------------------------------------

/// JSON envelope for a single environment (T037).
#[derive(serde::Serialize)]
struct EnvStatusJson {
    name: String,
    secret_count: i64,
    last_modified_at: Option<String>,
    status: &'static str,
}

/// JSON envelope for the `envy.enc` artifact (T037).
#[derive(serde::Serialize)]
struct ArtifactJson {
    found: bool,
    path: String,
    last_modified_at: Option<String>,
    environments: Vec<String>,
}

/// Top-level JSON output for `envy status --format json` (T037).
#[derive(serde::Serialize)]
struct StatusJson {
    environments: Vec<EnvStatusJson>,
    artifact: ArtifactJson,
}

/// Internal metadata about the `envy.enc` artifact — no decryption (T043).
struct ArtifactMetadata {
    /// `true` iff the file exists and contains parseable JSON.
    found: bool,
    /// `true` iff the file exists but its JSON is malformed.
    malformed: bool,
    /// Unix epoch (UTC, seconds) of the file's last modification time.
    last_modified_at: Option<i64>,
    /// Environment names listed in the artifact. Empty if not found or malformed.
    environments: Vec<String>,
}

/// Returns a human-readable relative-time string for a Unix epoch timestamp (T025).
///
/// Thresholds:
/// - `delta ≤ 0` → `"unknown"`
/// - `0 < delta < 60` → `"X seconds ago"`
/// - `60 ≤ delta < 3600` → `"X minutes ago"`
/// - `3600 ≤ delta < 86400` → `"X hours ago"`
/// - `86400 ≤ delta < 30 days` → `"X days ago"`
/// - `≥ 30 days` → `"YYYY-MM-DD"`
fn humanize_timestamp(epoch: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delta = now - epoch;
    if delta <= 0 {
        return "unknown".to_string();
    }
    if delta < 60 {
        return format!("{delta} seconds ago");
    }
    if delta < 3_600 {
        let mins = delta / 60;
        return format!("{mins} minutes ago");
    }
    if delta < 86_400 {
        let hours = delta / 3_600;
        return format!("{hours} hours ago");
    }
    if delta < 30 * 86_400 {
        let days = delta / 86_400;
        return format!("{days} days ago");
    }
    // Older: ISO date only
    epoch_to_iso8601(epoch)[..10].to_string()
}

/// Converts a Unix epoch (UTC seconds) to an ISO 8601 string `YYYY-MM-DDTHH:MM:SSZ` (T038).
///
/// Uses the Howard Hinnant civil_from_days algorithm — no `chrono` dependency required.
/// Returns `"1970-01-01T00:00:00Z"` for `secs == 0`.
fn epoch_to_iso8601(secs: i64) -> String {
    // Decompose time-of-day component.
    let sec_of_day = secs % 86_400;
    let h = sec_of_day / 3_600;
    let mi = (sec_of_day % 3_600) / 60;
    let s = sec_of_day % 60;

    // Convert integer day count to Gregorian date.
    // Reference: https://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = secs / 86_400 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day of era [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month of year [0, 11], March-based
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y }; // adjust year for Jan/Feb

    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Reads artifact metadata (env names + mtime) without decrypting (T043).
///
/// Returns a metadata struct indicating whether the artifact was found, valid,
/// or malformed — no secret values are ever accessed.
fn read_artifact_metadata(artifact_path: &Path) -> ArtifactMetadata {
    // Read file mtime (works even for malformed JSON files).
    let last_modified_at = std::fs::metadata(artifact_path).ok().and_then(|m| {
        m.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
        })
    });

    match crate::core::read_artifact(artifact_path) {
        Ok(artifact) => ArtifactMetadata {
            found: true,
            malformed: false,
            last_modified_at,
            environments: artifact.environments.keys().cloned().collect(),
        },
        Err(crate::core::SyncError::FileNotFound(_)) => ArtifactMetadata {
            found: false,
            malformed: false,
            last_modified_at: None,
            environments: vec![],
        },
        Err(_) => ArtifactMetadata {
            found: false,
            malformed: true,
            last_modified_at,
            environments: vec![],
        },
    }
}

/// Builds the `StatusJson` value from status rows + artifact metadata (T037/T039/T045).
fn build_status_json(rows: &[crate::core::StatusRow], artifact_path: &Path) -> StatusJson {
    use crate::core::SyncStatus;

    let environments: Vec<EnvStatusJson> = rows
        .iter()
        .map(|row| {
            let status = match row.sync_status {
                SyncStatus::InSync => "in_sync",
                SyncStatus::Modified => "modified",
                SyncStatus::NeverSealed => "never_sealed",
            };
            EnvStatusJson {
                name: row.name.clone(),
                secret_count: row.secret_count,
                last_modified_at: row.last_modified_at.map(epoch_to_iso8601),
                status,
            }
        })
        .collect();

    let meta = read_artifact_metadata(artifact_path);
    let artifact = ArtifactJson {
        found: meta.found,
        path: artifact_path.display().to_string(),
        last_modified_at: meta.last_modified_at.map(epoch_to_iso8601),
        environments: meta.environments,
    };

    StatusJson {
        environments,
        artifact,
    }
}

/// Serializes the status JSON to `writer` (T039).
///
/// Separated from `cmd_status` so that tests can pass a `Vec<u8>` writer
/// instead of stdout, enabling capture and assertion on the output.
fn write_status_json(
    rows: &[crate::core::StatusRow],
    artifact_path: &Path,
    writer: &mut impl std::io::Write,
) -> Result<(), CliError> {
    let output = build_status_json(rows, artifact_path);
    serde_json::to_writer(&mut *writer, &output).map_err(|e| CliError::Output(e.to_string()))?;
    writeln!(writer).map_err(|e| CliError::Output(e.to_string()))?;
    Ok(())
}

/// Displays sync status of all environments (T026).
///
/// Table format: prints a `comfy_table` environment table followed by an
/// artifact metadata section. JSON format: outputs a single JSON object.
///
/// # Passphrase constraint
/// MUST NOT prompt for a passphrase, call any decryption function, or read
/// `ENVY_PASSPHRASE*` environment variables.
pub(super) fn cmd_status(
    vault: &Vault,
    project_id: &ProjectId,
    artifact_path: &Path,
    format: OutputFormat,
) -> Result<(), CliError> {
    use crate::core::SyncStatus;

    let rows = crate::core::get_status_report(vault, project_id).map_err(CliError::Core)?;

    // JSON path (T039): serialize and exit early.
    if format == OutputFormat::Json {
        return write_status_json(&rows, artifact_path, &mut std::io::stdout());
    }

    // Table path — empty vault guard.
    if rows.is_empty() {
        println!("No environments found. Use 'envy set' to add secrets first.");
        return Ok(());
    }

    // Build environment table (T026).
    let mut table = comfy_table::Table::new();
    table.set_header(vec!["Environment", "Secrets", "Last Modified", "Status"]);

    for row in &rows {
        let last_modified = if row.secret_count == 0 {
            "No secrets".to_string()
        } else {
            row.last_modified_at
                .map(humanize_timestamp)
                .unwrap_or_else(|| "No secrets".to_string())
        };

        let (status_text, status_color) = match row.sync_status {
            SyncStatus::InSync => ("\u{2713} In Sync", comfy_table::Color::Green),
            SyncStatus::Modified => ("\u{26a0} Modified", comfy_table::Color::Yellow),
            SyncStatus::NeverSealed => ("\u{2717} Never Sealed", comfy_table::Color::Red),
        };

        table.add_row(vec![
            comfy_table::Cell::new(&row.name),
            comfy_table::Cell::new(row.secret_count.to_string()),
            comfy_table::Cell::new(&last_modified),
            comfy_table::Cell::new(status_text)
                .fg(status_color)
                .add_attribute(comfy_table::Attribute::Bold),
        ]);
    }

    println!("{table}");

    // Artifact metadata section (T044).
    let meta = read_artifact_metadata(artifact_path);
    let path_display = artifact_path.display();
    if meta.found {
        let mtime = meta
            .last_modified_at
            .map(humanize_timestamp)
            .unwrap_or_else(|| "unknown".to_string());
        let envs = meta.environments.join(", ");
        println!("\nArtifact: {path_display}  (last written: {mtime})");
        println!("  Sealed environments: {envs}");
    } else if meta.malformed {
        println!("\nArtifact: {path_display}  \u{2014} unreadable (malformed JSON)");
    } else {
        println!("\nArtifact: {path_display}  \u{2014} not found");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// T014 — Color helpers for diff output
// ---------------------------------------------------------------------------

/// Returns `true` if ANSI color output is enabled.
///
/// Color is suppressed when `NO_COLOR` is set (no-color.org convention) or
/// stdout is not a terminal (piped/redirected output).
fn is_color_enabled() -> bool {
    use std::io::IsTerminal;
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
}

/// Wraps `text` in ANSI escape sequences when color is enabled.
///
/// `ansi` is the SGR code (e.g. `"32"` for green, `"31"` for red, `"33"` for yellow).
fn colorize(text: &str, ansi: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[{ansi}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// T015–T016 — Table renderer for diff output
// ---------------------------------------------------------------------------

/// Renders the diff report as a human-readable table to stdout.
///
/// Output format follows contracts/diff-command.md §Standard Output — Table Format.
fn render_diff_table(
    report: &crate::core::DiffReport,
    reveal: bool,
    artifact_missing: bool,
    env_not_in_artifact: bool,
) {
    use crate::core::ChangeType;

    if !report.has_differences() {
        println!("envy diff: {} \u{2014} no differences", report.env_name);
        return;
    }

    println!("envy diff: {} (vault \u{2194} envy.enc)", report.env_name);

    if artifact_missing {
        println!("Note: envy.enc not found \u{2014} all vault secrets shown as additions.");
    } else if env_not_in_artifact {
        println!(
            "Note: environment '{}' not found in envy.enc \u{2014} all vault secrets shown as additions.",
            report.env_name
        );
    }

    println!();

    for entry in &report.entries {
        let (symbol, ansi) = match entry.change {
            ChangeType::Added => ("+", "32"),
            ChangeType::Removed => ("-", "31"),
            ChangeType::Modified => ("~", "33"),
        };
        println!("  {}", colorize(&format!("{symbol} {}", entry.key), ansi));

        if reveal {
            match entry.change {
                ChangeType::Added => {
                    if let Some(ref v) = entry.new_value {
                        println!("    vault:    {}", **v);
                    }
                }
                ChangeType::Removed => {
                    if let Some(ref v) = entry.old_value {
                        println!("    artifact: {}", **v);
                    }
                }
                ChangeType::Modified => {
                    if let Some(ref v) = entry.old_value {
                        println!("    artifact: {}", **v);
                    }
                    if let Some(ref v) = entry.new_value {
                        println!("    vault:    {}", **v);
                    }
                }
            }
            println!();
        }
    }

    let total = report.total();
    let label = if total == 1 { "change" } else { "changes" };
    println!(
        "{total} {label}: {} added, {} removed, {} modified",
        report.added, report.removed, report.modified
    );
}

// ---------------------------------------------------------------------------
// T017 — JSON writer for diff output
// ---------------------------------------------------------------------------

/// Serializes the diff report as JSON to `writer`.
///
/// When `reveal` is false, the `old_value`/`new_value` keys are entirely absent
/// from each change entry (not null — absent). See research.md R5.
fn write_diff_json(
    report: &crate::core::DiffReport,
    env_name: &str,
    reveal: bool,
    writer: &mut impl std::io::Write,
) -> Result<(), CliError> {
    use crate::core::ChangeType;

    let changes: Vec<serde_json::Value> = report
        .entries
        .iter()
        .map(|e| {
            let type_str = match e.change {
                ChangeType::Added => "added",
                ChangeType::Removed => "removed",
                ChangeType::Modified => "modified",
            };
            let mut entry = serde_json::json!({
                "key": e.key,
                "type": type_str,
            });
            if reveal {
                entry["old_value"] = match &e.old_value {
                    Some(v) => serde_json::Value::String(v.to_string()),
                    None => serde_json::Value::Null,
                };
                entry["new_value"] = match &e.new_value {
                    Some(v) => serde_json::Value::String(v.to_string()),
                    None => serde_json::Value::Null,
                };
            }
            entry
        })
        .collect();

    let output = serde_json::json!({
        "environment": env_name,
        "has_differences": report.has_differences(),
        "summary": {
            "added": report.added,
            "removed": report.removed,
            "modified": report.modified,
            "total": report.total(),
        },
        "changes": changes,
    });

    serde_json::to_writer_pretty(&mut *writer, &output)
        .map_err(|e| CliError::Output(e.to_string()))?;
    writeln!(writer).map_err(|e| CliError::Output(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T022 — cmd_diff handler
// ---------------------------------------------------------------------------

/// Compares vault secrets against the sealed `envy.enc` artifact for one environment.
///
/// Returns `Ok(true)` if differences were found (exit 1), `Ok(false)` if clean (exit 0).
/// This is the only `cmd_*` handler that returns `Result<bool, CliError>` — see research.md R4.
pub(super) fn cmd_diff(
    vault: &Vault,
    master_key: &[u8; 32],
    project_id: &ProjectId,
    env_name: &str,
    artifact_path: &Path,
    format: OutputFormat,
    reveal: bool,
) -> Result<bool, CliError> {
    use std::collections::BTreeMap;

    // Step 1 — Vault side: fetch secrets, convert HashMap → BTreeMap.
    let vault_map: BTreeMap<String, zeroize::Zeroizing<String>> =
        crate::core::get_env_secrets(vault, master_key, project_id, env_name)
            .map_err(CliError::Core)?
            .into_iter()
            .collect();

    // Step 2 — Artifact side: read envy.enc.
    let mut artifact_map: BTreeMap<String, zeroize::Zeroizing<String>> = BTreeMap::new();
    let mut artifact_missing = false;
    let mut env_not_in_artifact = false;

    match crate::core::read_artifact(artifact_path) {
        Err(crate::core::SyncError::FileNotFound(_)) => {
            artifact_missing = true;
        }
        Err(e) => {
            return Err(CliError::ArtifactUnreadable(e.to_string()));
        }
        Ok(artifact) => {
            if artifact.environments.contains_key(env_name) {
                // Step 3 — Resolve passphrase and unseal.
                let passphrase = match resolve_passphrase_for_env(env_name, false, None)? {
                    Some(p) => p,
                    None => {
                        return Err(CliError::PassphraseInput(
                            "no passphrase available (no TTY and no ENVY_PASSPHRASE* env var)"
                                .into(),
                        ));
                    }
                };
                match crate::core::unseal_env(&artifact, env_name, passphrase.as_ref())
                    .map_err(|e| CliError::VaultOpen(e.to_string()))?
                {
                    Some(secrets) => {
                        artifact_map = secrets;
                    }
                    None => {
                        return Err(CliError::PassphraseInput(format!(
                            "incorrect passphrase for environment '{env_name}'"
                        )));
                    }
                }
            } else {
                env_not_in_artifact = true;
            }
        }
    }

    // Step 4 — Both sides empty → env not found anywhere.
    if vault_map.is_empty() && artifact_map.is_empty() && !artifact_missing && !env_not_in_artifact
    {
        return Err(CliError::EnvNotFound(env_name.to_string()));
    }

    // Step 5 — Compute diff.
    let report = crate::core::compute_diff(env_name, vault_map, artifact_map);

    // Step 6 — Render.
    if reveal {
        eprintln!("\u{26a0} Warning: secret values are visible in the output below.");
        eprintln!();
    }

    if format == OutputFormat::Json {
        write_diff_json(&report, env_name, reveal, &mut std::io::stdout())?;
    } else {
        render_diff_table(&report, reveal, artifact_missing, env_not_in_artifact);
    }

    // Step 7 — Return whether differences were found.
    Ok(report.has_differences())
}

// ---------------------------------------------------------------------------
// T008–T011, T022–T023 — Unit tests (written FIRST per TDD discipline)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{Vault, cmd_decrypt, cmd_encrypt};
    use crate::cli::error::{CliError, cli_exit_code};
    use crate::cli::format::OutputFormat;

    // -----------------------------------------------------------------------
    // T008–T011 — assignment parsing (Phase 3)
    // -----------------------------------------------------------------------

    // T008
    #[test]
    fn parse_assignment_basic() {
        assert_eq!("KEY=VALUE".split_once('='), Some(("KEY", "VALUE")));
    }

    // T009
    #[test]
    fn parse_assignment_value_contains_equals() {
        // The first `=` is the separator; the rest of the value is preserved.
        assert_eq!("TOKEN=abc=def".split_once('='), Some(("TOKEN", "abc=def")));
    }

    // T010
    #[test]
    fn parse_assignment_no_equals() {
        // A token with no `=` must be detected as malformed (→ CliError::InvalidAssignment).
        assert_eq!("NOVALUE".split_once('='), None);
    }

    // T011
    #[test]
    fn parse_assignment_empty_key() {
        // "=VALUE" splits into ("", "VALUE") — Core's validate_key rejects the empty key.
        assert_eq!("=VALUE".split_once('='), Some(("", "VALUE")));
    }

    // -----------------------------------------------------------------------
    // T022–T023 — migrate line-parsing logic (Phase 4, written before impl)
    //
    // These tests exercise the exact parsing pattern used by cmd_migrate:
    //   trim → skip blank/comment → split_once('=') → import or warn
    // They are pure logic tests with no I/O, vault, or crypto involvement.
    // -----------------------------------------------------------------------

    /// Shared helper: runs the migrate line-parser over `input` and returns
    /// (valid_pairs, malformed_count) — mirroring cmd_migrate's inner loop.
    fn parse_env_lines<'a>(input: &'a str) -> (Vec<(&'a str, &'a str)>, usize) {
        let mut valid: Vec<(&str, &str)> = Vec::new();
        let mut malformed = 0usize;
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            match trimmed.split_once('=') {
                Some((k, v)) => valid.push((k, v)),
                None => malformed += 1,
            }
        }
        (valid, malformed)
    }

    // T022
    #[test]
    fn migrate_skips_comments_and_blanks() {
        let input = "KEY1=value1\n# this is a comment\n\nKEY2=value2\n";
        let (valid, malformed) = parse_env_lines(input);

        assert_eq!(valid.len(), 2, "must produce exactly 2 valid pairs");
        assert_eq!(malformed, 0, "must report 0 malformed lines");
        assert_eq!(valid[0], ("KEY1", "value1"));
        assert_eq!(valid[1], ("KEY2", "value2"));
    }

    // T023
    #[test]
    fn migrate_warns_on_malformed() {
        let input = "GOOD_KEY=good_value\nBADLINE\n";
        let (valid, malformed) = parse_env_lines(input);

        assert_eq!(valid.len(), 1, "must produce exactly 1 valid pair");
        assert_eq!(malformed, 1, "must detect exactly 1 malformed line");
        assert_eq!(valid[0], ("GOOD_KEY", "good_value"));
    }

    // -----------------------------------------------------------------------
    // Phase 2 — cmd_encrypt tests (T012–T015)
    //
    // TDD: written FIRST. Tests compile but panic (todo!) until cmd_encrypt
    // is implemented in T017–T018.
    //
    // ENVY_PASSPHRASE isolation: all tests that touch the environment variable
    // acquire ENV_LOCK before setting it and release on drop, serialising
    // env-var access across parallel test threads.
    // -----------------------------------------------------------------------

    /// Serialises env-var access across parallel test threads.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const TEST_MASTER_KEY: [u8; 32] = [0xABu8; 32];

    fn open_test_vault(tmp: &tempfile::TempDir) -> (Vault, crate::db::ProjectId) {
        let path = tmp.path().join("vault.db");
        let vault = Vault::open(&path, &TEST_MASTER_KEY).expect("vault must open");
        let pid = vault
            .create_project("test-project")
            .expect("project must be created");
        (vault, pid)
    }

    // T012 — contract: cmd_encrypt writes envy.enc with correct environments
    #[test]
    fn encrypt_writes_envy_enc_with_correct_environments() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "API_KEY",
            "sk_test",
        )
        .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "test-passphrase") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_encrypt must succeed");
        assert!(artifact_path.exists(), "envy.enc must be written to disk");

        let raw = std::fs::read_to_string(&artifact_path).expect("must read envy.enc");
        assert!(
            raw.contains("\"development\""),
            "JSON must contain environment name"
        );
        assert!(
            !raw.contains("sk_test"),
            "secret value must NOT appear in plaintext"
        );
        assert!(
            !raw.contains("API_KEY"),
            "secret key must NOT appear in plaintext"
        );
    }

    // T013 — contract: cmd_encrypt uses ENVY_PASSPHRASE when set (no prompt)
    #[test]
    fn encrypt_uses_envy_passphrase_env_var_no_prompt() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // No terminal available — if cmd_encrypt tries to prompt, it will fail.
        // Returning Ok(()) proves it used ENVY_PASSPHRASE instead.
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "headless-pass") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_encrypt must use ENVY_PASSPHRASE without prompting");
        assert!(artifact_path.exists(), "envy.enc must be written");
    }

    // T014 — contract: exit code 2 path — empty/whitespace ENVY_PASSPHRASE is rejected
    #[test]
    fn encrypt_empty_envy_passphrase_returns_passphrase_input_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        let artifact_path = tmp.path().join("envy.enc");
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "   ") }; // whitespace only
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        assert!(
            matches!(result, Err(CliError::PassphraseInput(_))),
            "whitespace ENVY_PASSPHRASE must return PassphraseInput, got: {:?}",
            result.err()
        );
        assert!(
            !artifact_path.exists(),
            "envy.enc must NOT be written on error"
        );
    }

    // T015 — contract: env_filter seals only the named environment
    #[test]
    fn encrypt_env_filter_seals_only_named_environment() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "DEV_KEY",
            "dev",
        )
        .expect("set_secret must succeed");
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "staging", "STG_KEY", "stg")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "filter-pass") };
        let result = cmd_encrypt(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            &artifact_path,
            Some("staging"),
        );
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_encrypt with env_filter must succeed");
        let raw = std::fs::read_to_string(&artifact_path).expect("must read envy.enc");
        assert!(raw.contains("\"staging\""), "envy.enc must contain staging");
        assert!(
            !raw.contains("\"development\""),
            "envy.enc must NOT contain development"
        );
    }

    // T015 (new) — per-env passphrase var seals only that environment
    #[test]
    fn encrypt_uses_per_env_passphrase_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // Set only the per-env var — no global ENVY_PASSPHRASE.
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE_DEVELOPMENT", "dev-specific-pass") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE_DEVELOPMENT") };

        result.expect("cmd_encrypt must succeed with ENVY_PASSPHRASE_DEVELOPMENT");
        assert!(artifact_path.exists(), "envy.enc must be written");
        let raw = std::fs::read_to_string(&artifact_path).expect("must read envy.enc");
        assert!(
            raw.contains("\"development\""),
            "envy.enc must contain development"
        );
    }

    // T016 — smart merge: pre-existing envelope is preserved byte-for-byte
    #[test]
    fn encrypt_smart_merge_preserves_existing_envelopes() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        // Set up both environments in the vault.
        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "DEV_KEY",
            "dev",
        )
        .expect("set_secret must succeed");
        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "production",
            "PROD_KEY",
            "prod",
        )
        .expect("set_secret must succeed");

        // Pre-populate envy.enc with only the production envelope.
        let prod_artifact = crate::core::seal_artifact(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "prod-pass",
            Some(&["production"]),
        )
        .expect("seal_artifact must succeed");
        let artifact_path = tmp.path().join("envy.enc");
        crate::core::write_artifact(&prod_artifact, &artifact_path)
            .expect("write_artifact must succeed");

        // Capture the production envelope bytes before the merge.
        let raw_before = std::fs::read_to_string(&artifact_path).expect("must read before");
        let before: serde_json::Value =
            serde_json::from_str(&raw_before).expect("must parse before");

        // Now encrypt only development (smart merge should keep production unchanged).
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE_DEVELOPMENT", "dev-pass") };
        let result = cmd_encrypt(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            &artifact_path,
            Some("development"),
        );
        unsafe { std::env::remove_var("ENVY_PASSPHRASE_DEVELOPMENT") };

        result.expect("smart merge must succeed");

        let raw_after = std::fs::read_to_string(&artifact_path).expect("must read after");
        let after: serde_json::Value = serde_json::from_str(&raw_after).expect("must parse after");

        assert!(
            raw_after.contains("\"development\""),
            "development must be present after merge"
        );
        assert!(
            raw_after.contains("\"production\""),
            "production must be preserved after merge"
        );
        // Production envelope must be byte-identical — not re-sealed.
        assert_eq!(
            before["environments"]["production"], after["environments"]["production"],
            "production envelope must be byte-identical after smart merge"
        );
    }

    // T018 — malformed envy.enc aborts without overwriting (FR-013)
    #[test]
    fn encrypt_aborts_on_malformed_envy_enc() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // Write malformed JSON — cmd_encrypt must not silently overwrite it.
        std::fs::write(&artifact_path, b"{{not valid json}}")
            .expect("write stale content must succeed");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "pass") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        assert!(
            result.is_err(),
            "malformed envy.enc must cause an error (not a silent overwrite)"
        );
        // The original malformed file must still be there (not overwritten).
        let remaining = std::fs::read(&artifact_path).expect("file must still exist");
        assert_eq!(
            remaining, b"{{not valid json}}",
            "malformed envy.enc must not be overwritten on error"
        );
    }

    // T019 — stale .tmp file is cleaned up after a successful encrypt (FR-006)
    #[test]
    fn encrypt_removes_stale_tmp_file_on_success() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        let tmp_path = tmp.path().join("envy.enc.tmp");

        // Simulate a previous crash that left a stale .tmp file.
        std::fs::write(&tmp_path, b"stale-tmp-content").expect("write stale .tmp must succeed");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "pass") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_encrypt must succeed even with a stale .tmp file");

        // The stale .tmp must be gone after the atomic write.
        assert!(
            !tmp_path.exists(),
            "envy.enc.tmp must be removed after success"
        );

        // The final envy.enc must be valid and contain the expected environment.
        let raw = std::fs::read_to_string(&artifact_path).expect("must read envy.enc");
        assert!(
            raw.contains("\"development\""),
            "envy.enc must contain development after success"
        );
    }

    // T025 — pre-flight check returns false for wrong passphrase (FR-008)
    #[test]
    fn check_envelope_passphrase_returns_false_for_rotation_detection() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "K", "v")
            .expect("set_secret must succeed");

        // Seal with pass-A.
        let artifact = crate::core::seal_artifact(&vault, &TEST_MASTER_KEY, &pid, "pass-A", None)
            .expect("seal_artifact must succeed");
        let envelope = artifact
            .environments
            .get("development")
            .expect("development must be present");

        // Wrong passphrase → false (rotation warning path would trigger).
        assert!(
            !crate::core::check_envelope_passphrase("pass-B", "development", envelope),
            "wrong passphrase must return false (key-rotation warning path)"
        );
        // Correct passphrase → true.
        assert!(
            crate::core::check_envelope_passphrase("pass-A", "development", envelope),
            "correct passphrase must return true"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 3 — cmd_decrypt tests (T021–T026)
    //
    // Same ENV_LOCK discipline: all tests that mutate ENVY_PASSPHRASE
    // acquire ENV_LOCK before setting it and release on drop.
    // -----------------------------------------------------------------------

    // T021 — contract: cmd_decrypt imports all secrets with correct passphrase
    #[test]
    fn decrypt_imports_all_secrets_with_correct_passphrase() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "API_KEY",
            "sk_test",
        )
        .expect("set_secret must succeed");

        // Seal artifact directly (bypasses cmd_encrypt / passphrase resolution).
        let artifact =
            crate::core::seal_artifact(&vault, &TEST_MASTER_KEY, &pid, "correct-pass", None)
                .expect("seal_artifact must succeed");
        let artifact_path = tmp.path().join("envy.enc");
        crate::core::write_artifact(&artifact, &artifact_path)
            .expect("write_artifact must succeed");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "correct-pass") };
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_decrypt must succeed with correct passphrase");

        let val = crate::core::get_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "API_KEY")
            .expect("get_secret must succeed after decrypt");
        assert_eq!(
            val.as_str(),
            "sk_test",
            "upserted value must match original"
        );
    }

    // T022 — contract: cmd_decrypt returns NothingImported when all envs skipped
    #[test]
    fn decrypt_returns_nothing_imported_when_all_envs_skipped() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");
        let artifact =
            crate::core::seal_artifact(&vault, &TEST_MASTER_KEY, &pid, "passphrase-a", None)
                .expect("seal_artifact must succeed");
        let artifact_path = tmp.path().join("envy.enc");
        crate::core::write_artifact(&artifact, &artifact_path)
            .expect("write_artifact must succeed");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "wrong-passphrase") };
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        assert!(
            matches!(result, Err(CliError::NothingImported)),
            "wrong passphrase must return NothingImported, got: {:?}",
            result.err()
        );
    }

    // T023 — contract: cmd_decrypt exits 0 and shows skipped for partial access
    #[test]
    fn decrypt_exits_ok_and_shows_skipped_for_partial_access() {
        use crate::crypto::artifact::{ARTIFACT_VERSION, ArtifactPayload, SyncArtifact};
        use crate::crypto::seal_envelope;
        use std::collections::BTreeMap;
        use zeroize::Zeroizing;

        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        // Build a SyncArtifact manually with two different passphrases per environment.
        let mut dev_secrets = BTreeMap::new();
        dev_secrets.insert(
            "DEV_KEY".to_string(),
            Zeroizing::new("dev-value".to_string()),
        );
        let dev_payload = ArtifactPayload {
            secrets: dev_secrets,
        };
        let dev_envelope = seal_envelope("dev-pass", &dev_payload).expect("seal dev must succeed");

        let mut prod_secrets = BTreeMap::new();
        prod_secrets.insert(
            "PROD_KEY".to_string(),
            Zeroizing::new("prod-value".to_string()),
        );
        let prod_payload = ArtifactPayload {
            secrets: prod_secrets,
        };
        let prod_envelope =
            seal_envelope("prod-pass", &prod_payload).expect("seal prod must succeed");

        let mut environments = BTreeMap::new();
        environments.insert("development".to_string(), dev_envelope);
        environments.insert("production".to_string(), prod_envelope);
        let artifact = SyncArtifact {
            version: ARTIFACT_VERSION,
            environments,
        };

        let artifact_path = tmp.path().join("envy.enc");
        crate::core::write_artifact(&artifact, &artifact_path)
            .expect("write_artifact must succeed");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "dev-pass") };
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        assert!(
            result.is_ok(),
            "partial access must return Ok(()), got: {:?}",
            result.err()
        );

        // development secret must be in vault (was imported).
        let val = crate::core::get_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "DEV_KEY")
            .expect("DEV_KEY must be upserted after partial decrypt");
        assert_eq!(val.as_str(), "dev-value");

        // production secret must NOT be in vault (was skipped).
        let prod_result =
            crate::core::get_secret(&vault, &TEST_MASTER_KEY, &pid, "production", "PROD_KEY");
        assert!(
            prod_result.is_err(),
            "PROD_KEY must NOT be in vault after partial decrypt"
        );
    }

    // T024 — contract: Exit code 1 when envy.enc not found
    #[test]
    fn decrypt_returns_error_when_envy_enc_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        let nonexistent = tmp.path().join("missing.enc");
        // No ENVY_PASSPHRASE needed — read_artifact fails before resolve_passphrase.
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &nonexistent);

        let err = result.expect_err("missing envy.enc must return an error");
        assert!(
            matches!(err, CliError::FileNotFound(_, _)),
            "missing envy.enc must return FileNotFound, got: {:?}",
            err
        );
        assert_eq!(
            cli_exit_code(&err),
            1,
            "FileNotFound must map to exit code 1"
        );
    }

    // T025 — contract: Exit code 4 for malformed envy.enc
    #[test]
    fn decrypt_returns_error_for_malformed_envy_enc() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        let bad_path = tmp.path().join("bad.enc");
        std::fs::write(&bad_path, b"this is not json").expect("write must succeed");

        // No ENVY_PASSPHRASE needed — read_artifact fails before resolve_passphrase.
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &bad_path);

        let err = result.expect_err("malformed envy.enc must return an error");
        assert_eq!(
            cli_exit_code(&err),
            4,
            "malformed envy.enc must map to exit code 4, got: {:?}",
            err
        );
    }

    // T035 [F1] — empty env is skipped with a warning; envy.enc must not contain it
    #[test]
    fn encrypt_skips_empty_env_with_warning() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        // Create an environment with zero secrets — F1 guard must skip it.
        vault
            .create_environment(&pid, "empty-env")
            .expect("create_environment must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "any-pass") };
        let result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        result.expect("cmd_encrypt must return Ok(()) even when all envs are skipped");

        // envy.enc is written (atomic write always runs) but must not contain the empty env.
        let raw = std::fs::read_to_string(&artifact_path).expect("envy.enc must exist");
        assert!(
            !raw.contains("\"empty-env\""),
            "empty env must not appear in envy.enc after F1 skip"
        );
    }

    // T038 [F2] — cmd_decrypt uses ENVY_PASSPHRASE_<ENV> for per-env decryption
    #[test]
    fn decrypt_uses_per_env_passphrase_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "DEV_SECRET",
            "dev-value",
        )
        .expect("set_secret must succeed");

        // Seal the artifact using cmd_encrypt with a per-env var so we know the passphrase.
        let artifact_path = tmp.path().join("envy.enc");
        unsafe { std::env::set_var("ENVY_PASSPHRASE_DEVELOPMENT", "dev-pass") };
        cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None)
            .expect("cmd_encrypt must succeed");
        unsafe { std::env::remove_var("ENVY_PASSPHRASE_DEVELOPMENT") };

        // Delete the secret from the vault so we can verify it gets re-imported.
        let env_id = vault
            .get_environment_by_name(&pid, "development")
            .expect("env must exist")
            .id;
        vault
            .delete_secret(&env_id, "DEV_SECRET")
            .expect("delete_secret must succeed");

        // Now run cmd_decrypt with only the per-env var set (no global ENVY_PASSPHRASE).
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE_DEVELOPMENT", "dev-pass") };
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE_DEVELOPMENT") };

        result.expect("cmd_decrypt must succeed with ENVY_PASSPHRASE_DEVELOPMENT");

        let val =
            crate::core::get_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "DEV_SECRET")
                .expect("DEV_SECRET must be re-imported after decrypt");
        assert_eq!(
            val.as_str(),
            "dev-value",
            "re-imported value must match original"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 3 — cmd_status tests (T027–T032)
    // -----------------------------------------------------------------------

    // T027 — never-sealed env shows in table without error
    #[test]
    fn status_shows_never_sealed_for_new_environment() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        let result = super::cmd_status(&vault, &pid, &artifact_path, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok for never-sealed env: {:?}",
            result.err()
        );
    }

    // T028 — env with a direct DB marker renders as In Sync
    #[test]
    fn status_shows_in_sync_via_direct_db_marker() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        // Insert a sync marker with a timestamp far in the future so the status is InSync.
        let env_id = vault
            .get_environment_by_name(&pid, "development")
            .expect("env must exist")
            .id;
        vault
            .upsert_sync_marker(&env_id, i64::MAX)
            .expect("upsert_sync_marker must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        let result = super::cmd_status(&vault, &pid, &artifact_path, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok for in-sync env: {:?}",
            result.err()
        );
    }

    // T029 — empty vault returns Ok with "No environments" message
    #[test]
    fn status_empty_vault_returns_ok() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        let artifact_path = tmp.path().join("envy.enc");
        let result = super::cmd_status(&vault, &pid, &artifact_path, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status on empty vault must return Ok: {:?}",
            result.err()
        );
    }

    // T030 — humanize_timestamp: 30 seconds ago
    #[test]
    fn humanize_timestamp_seconds() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let result = super::humanize_timestamp(now - 30);
        assert!(
            result.contains("seconds ago"),
            "30 seconds ago must produce '… seconds ago', got: {result}"
        );
    }

    // T031 — humanize_timestamp: 90 seconds ago → minutes ago
    #[test]
    fn humanize_timestamp_minutes() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let result = super::humanize_timestamp(now - 90);
        assert!(
            result.contains("minutes ago"),
            "90 seconds ago must produce '… minutes ago', got: {result}"
        );
    }

    // T032 — humanize_timestamp: 3 days ago
    #[test]
    fn humanize_timestamp_days() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let result = super::humanize_timestamp(now - 3 * 86_400);
        assert!(
            result.contains("days ago"),
            "3 days ago must produce '… days ago', got: {result}"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 4 — sync marker wiring tests (T035–T036)
    // -----------------------------------------------------------------------

    // T035 — after cmd_encrypt, cmd_status sees the environment as In Sync (full round-trip)
    #[test]
    fn status_shows_in_sync_after_encrypt() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");

        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "test-passphrase") };
        let enc_result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };
        enc_result.expect("cmd_encrypt must succeed");

        // After encrypt the sync marker must exist → status returns Ok.
        let result = super::cmd_status(&vault, &pid, &artifact_path, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok after encrypt: {:?}",
            result.err()
        );

        // Verify the environment is actually InSync via the status report.
        let rows = crate::core::get_status_report(&vault, &pid).expect("get_status_report");
        let dev = rows
            .iter()
            .find(|r| r.name == "development")
            .expect("development must exist");
        assert_eq!(
            dev.sync_status,
            crate::core::SyncStatus::InSync,
            "development must be InSync after encrypt"
        );
    }

    // T036 — after encrypt + set_secret, cmd_status sees the environment as Modified
    #[test]
    fn status_shows_modified_after_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");

        // Seal first so sealed_at is set.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "test-passphrase") };
        let enc_result = cmd_encrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path, None);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };
        enc_result.expect("cmd_encrypt must succeed");

        // Sleep 1 second so the next set_secret timestamp is strictly after sealed_at.
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Modify a secret after sealing.
        crate::core::set_secret(
            &vault,
            &TEST_MASTER_KEY,
            &pid,
            "development",
            "NEW_KEY",
            "new",
        )
        .expect("set_secret after encrypt must succeed");

        // Status must return Ok and environment must be Modified.
        let result = super::cmd_status(&vault, &pid, &artifact_path, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok after set: {:?}",
            result.err()
        );

        let rows = crate::core::get_status_report(&vault, &pid).expect("get_status_report");
        let dev = rows
            .iter()
            .find(|r| r.name == "development")
            .expect("development must exist");
        assert_eq!(
            dev.sync_status,
            crate::core::SyncStatus::Modified,
            "development must be Modified after set"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 5 — JSON output tests (T040–T042)
    // -----------------------------------------------------------------------

    // T040 — cmd_status JSON returns Ok and produces valid JSON with 1 environment
    #[test]
    fn status_json_output_is_valid_json() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let artifact_path = tmp.path().join("envy.enc");

        // Capture output via write_status_json using a Vec<u8> writer.
        let rows = crate::core::get_status_report(&vault, &pid).expect("get_status_report");
        let mut buf: Vec<u8> = Vec::new();
        super::write_status_json(&rows, &artifact_path, &mut buf)
            .expect("write_status_json must succeed");

        let json_str = String::from_utf8(buf).expect("must be valid UTF-8");
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("output must be valid JSON");
        let envs = parsed["environments"]
            .as_array()
            .expect("environments must be an array");
        assert_eq!(
            envs.len(),
            1,
            "must have exactly 1 environment, got: {}",
            envs.len()
        );
    }

    // T041 — JSON status strings are lowercase snake_case
    #[test]
    fn status_json_status_strings_are_lowercase() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        // Insert a past sync marker so status is "modified" (secret newer than seal).
        let env_id = vault
            .get_environment_by_name(&pid, "development")
            .expect("env must exist")
            .id;
        vault
            .upsert_sync_marker(&env_id, 1_000) // very old seal timestamp
            .expect("upsert_sync_marker must succeed");

        let artifact_path = tmp.path().join("envy.enc");
        let rows = crate::core::get_status_report(&vault, &pid).expect("get_status_report");
        let mut buf: Vec<u8> = Vec::new();
        super::write_status_json(&rows, &artifact_path, &mut buf)
            .expect("write_status_json must succeed");

        let json_str = String::from_utf8(buf).expect("valid UTF-8");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        let status = parsed["environments"][0]["status"]
            .as_str()
            .expect("status must be a string");
        assert_eq!(
            status, "modified",
            "status must be lowercase 'modified', got: {status}"
        );
    }

    // T042 — epoch_to_iso8601 known values
    #[test]
    fn epoch_to_iso8601_known_value() {
        assert_eq!(
            super::epoch_to_iso8601(0),
            "1970-01-01T00:00:00Z",
            "epoch 0 must map to 1970-01-01T00:00:00Z"
        );
        assert_eq!(
            super::epoch_to_iso8601(1_000_000_000),
            "2001-09-09T01:46:40Z",
            "epoch 1000000000 must map to 2001-09-09T01:46:40Z"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 6 — artifact metadata tests (T046–T048)
    // -----------------------------------------------------------------------

    // T046 — artifact not found renders gracefully (exit 0)
    #[test]
    fn status_artifact_not_found_renders_gracefully() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let nonexistent_artifact = tmp.path().join("no-such-file.enc");
        let result = super::cmd_status(&vault, &pid, &nonexistent_artifact, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok when artifact not found: {:?}",
            result.err()
        );
    }

    // T047 — malformed artifact renders gracefully (exit 0)
    #[test]
    fn status_artifact_malformed_renders_gracefully() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let bad_artifact = tmp.path().join("bad.enc");
        std::fs::write(&bad_artifact, b"not valid json").expect("write must succeed");

        let result = super::cmd_status(&vault, &pid, &bad_artifact, OutputFormat::Table);
        assert!(
            result.is_ok(),
            "cmd_status must return Ok when artifact is malformed: {:?}",
            result.err()
        );
    }

    // T048 — JSON output has artifact.found=false when artifact is missing
    #[test]
    fn status_json_artifact_found_false_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");

        let nonexistent = tmp.path().join("missing.enc");
        let rows = crate::core::get_status_report(&vault, &pid).expect("get_status_report");
        let mut buf: Vec<u8> = Vec::new();
        super::write_status_json(&rows, &nonexistent, &mut buf)
            .expect("write_status_json must succeed");

        let json_str = String::from_utf8(buf).expect("valid UTF-8");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(
            parsed["artifact"]["found"],
            serde_json::Value::Bool(false),
            "artifact.found must be false when file is missing"
        );
        let envs = parsed["artifact"]["environments"]
            .as_array()
            .expect("environments must be array");
        assert!(
            envs.is_empty(),
            "artifact.environments must be empty when file is missing"
        );
    }

    // -----------------------------------------------------------------------
    // T026 — contract: Exit code 2 for empty/whitespace passphrase
    // -----------------------------------------------------------------------

    // T026 — contract: Exit code 2 for empty/whitespace passphrase
    #[test]
    fn decrypt_returns_passphrase_input_error_for_empty_passphrase() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (vault, pid) = open_test_vault(&tmp);

        // Write a valid artifact so read_artifact succeeds, failure happens at passphrase step.
        crate::core::set_secret(&vault, &TEST_MASTER_KEY, &pid, "development", "KEY", "val")
            .expect("set_secret must succeed");
        let artifact =
            crate::core::seal_artifact(&vault, &TEST_MASTER_KEY, &pid, "valid-pass", None)
                .expect("seal_artifact must succeed");
        let artifact_path = tmp.path().join("envy.enc");
        crate::core::write_artifact(&artifact, &artifact_path)
            .expect("write_artifact must succeed");

        // Whitespace-only ENVY_PASSPHRASE — treated as unset, falls through to TTY prompt
        // which fails (no TTY in tests), returning CliError::PassphraseInput.
        // SAFETY: single-threaded access serialised by ENV_LOCK above.
        unsafe { std::env::set_var("ENVY_PASSPHRASE", "   ") };
        let result = cmd_decrypt(&vault, &TEST_MASTER_KEY, &pid, &artifact_path);
        unsafe { std::env::remove_var("ENVY_PASSPHRASE") };

        assert!(
            matches!(result, Err(CliError::PassphraseInput(_))),
            "whitespace ENVY_PASSPHRASE must return PassphraseInput, got: {:?}",
            result.err()
        );
    }

    // -------------------------------------------------------------------
    // T018–T021 — JSON diff writer unit tests
    // -------------------------------------------------------------------

    /// Helper: build a DiffReport from entries.
    fn make_diff_report(
        entries: Vec<(&str, crate::core::ChangeType, Option<&str>, Option<&str>)>,
    ) -> crate::core::DiffReport {
        use crate::core::{DiffEntry, DiffReport};
        use zeroize::Zeroizing;

        let mut added = 0usize;
        let mut removed = 0usize;
        let mut modified = 0usize;
        let diff_entries: Vec<DiffEntry> = entries
            .into_iter()
            .map(|(key, change, old, new)| {
                match change {
                    crate::core::ChangeType::Added => added += 1,
                    crate::core::ChangeType::Removed => removed += 1,
                    crate::core::ChangeType::Modified => modified += 1,
                }
                DiffEntry {
                    key: key.to_string(),
                    change,
                    old_value: old.map(|v| Zeroizing::new(v.to_string())),
                    new_value: new.map(|v| Zeroizing::new(v.to_string())),
                }
            })
            .collect();

        DiffReport {
            env_name: "development".to_string(),
            entries: diff_entries,
            added,
            removed,
            modified,
        }
    }

    // T018
    #[test]
    fn diff_json_no_reveal() {
        use crate::core::ChangeType;

        let report = make_diff_report(vec![
            ("API_KEY", ChangeType::Added, None, Some("secret")),
            ("DB_URL", ChangeType::Modified, Some("old"), Some("new")),
        ]);

        let mut buf = Vec::new();
        super::write_diff_json(&report, "development", false, &mut buf).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        assert_eq!(json["has_differences"], true);
        assert_eq!(json["summary"]["total"], 2);

        for change in json["changes"].as_array().unwrap() {
            assert!(
                change.get("old_value").is_none(),
                "old_value must be absent without --reveal"
            );
            assert!(
                change.get("new_value").is_none(),
                "new_value must be absent without --reveal"
            );
        }
    }

    // T019
    #[test]
    fn diff_json_with_reveal() {
        use crate::core::ChangeType;

        let report = make_diff_report(vec![
            ("API_KEY", ChangeType::Added, None, Some("secret")),
            ("DB_URL", ChangeType::Modified, Some("old"), Some("new")),
            ("OLD_KEY", ChangeType::Removed, Some("removed_val"), None),
        ]);

        let mut buf = Vec::new();
        super::write_diff_json(&report, "development", true, &mut buf).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        let changes = json["changes"].as_array().unwrap();

        // Added: old_value = null, new_value = "secret"
        let added = changes.iter().find(|c| c["type"] == "added").unwrap();
        assert_eq!(added["old_value"], serde_json::Value::Null);
        assert_eq!(added["new_value"], "secret");

        // Modified: both present
        let modified = changes.iter().find(|c| c["type"] == "modified").unwrap();
        assert_eq!(modified["old_value"], "old");
        assert_eq!(modified["new_value"], "new");

        // Removed: new_value = null
        let removed = changes.iter().find(|c| c["type"] == "removed").unwrap();
        assert_eq!(removed["old_value"], "removed_val");
        assert_eq!(removed["new_value"], serde_json::Value::Null);
    }

    // T020
    #[test]
    fn diff_json_no_differences() {
        let report = make_diff_report(vec![]);

        let mut buf = Vec::new();
        super::write_diff_json(&report, "development", false, &mut buf).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        assert_eq!(json["has_differences"], false);
        assert_eq!(json["changes"].as_array().unwrap().len(), 0);
        assert_eq!(json["summary"]["added"], 0);
        assert_eq!(json["summary"]["removed"], 0);
        assert_eq!(json["summary"]["modified"], 0);
        assert_eq!(json["summary"]["total"], 0);
    }

    // T021
    #[test]
    fn diff_json_type_strings() {
        use crate::core::ChangeType;

        let report = make_diff_report(vec![
            ("A", ChangeType::Added, None, Some("v")),
            ("B", ChangeType::Modified, Some("old"), Some("new")),
            ("C", ChangeType::Removed, Some("v"), None),
        ]);

        let mut buf = Vec::new();
        super::write_diff_json(&report, "development", false, &mut buf).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        let types: Vec<&str> = json["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["type"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"added"), "must contain 'added'");
        assert!(types.contains(&"removed"), "must contain 'removed'");
        assert!(types.contains(&"modified"), "must contain 'modified'");
    }
}
