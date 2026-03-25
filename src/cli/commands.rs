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

    // Step 3 — Ensure the vault directory exists, then open (or create) the vault.
    let vault_path = super::vault_path();
    if let Some(vault_dir) = vault_path.parent() {
        std::fs::create_dir_all(vault_dir)
            .map_err(|e| CliError::VaultOpen(format!("cannot create vault directory: {e}")))?;
    }
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

    // Step 4 — Upsert all imported secrets; individual failures are warnings, not errors.
    for (env_name, secrets) in &imported {
        for (key, value) in secrets {
            if let Err(e) = crate::core::set_secret(
                vault,
                master_key,
                project_id,
                env_name,
                key,
                value.as_ref(),
            ) {
                eprintln!("warning: failed to upsert {env_name}/{key}: {e}");
            }
        }
    }

    // Step 5 — Print success header.
    println!("Imported {} environment(s) from envy.enc", imported.len());

    // Step 6 — Progressive Disclosure: green ✓ for imported, yellow ⚠ dim for skipped.
    for (env_name, secrets) in &imported {
        println!(
            "  {}  {} ({} secret(s) upserted)",
            dialoguer::console::style("\u{2713}").green(),
            env_name,
            secrets.len()
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
// T008–T011, T022–T023 — Unit tests (written FIRST per TDD discipline)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{Vault, cmd_decrypt, cmd_encrypt};
    use crate::cli::error::{CliError, cli_exit_code};

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
}
