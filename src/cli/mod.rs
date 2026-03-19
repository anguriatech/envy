//! UI/CLI layer — argument parsing and output formatting (clap).
//!
//! This is the sole layer that owns the Vault connection lifecycle and fetches
//! the master key from the OS credential store. All business logic is delegated
//! to `core`. Must not call `db` or `crypto` directly, except for
//! `Vault::open` and `crypto::get_or_create_master_key`.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::core` for DB/crypto operations — use Core functions.
//! - MAY call `crate::db::Vault::open` and `crate::crypto::get_or_create_master_key`
//!   as the sole permitted infrastructure exceptions.

mod commands;
mod error;

use clap::{Parser, Subcommand};

pub use error::{CliError, cli_exit_code, core_exit_code, format_cli_error, format_core_error};

// ---------------------------------------------------------------------------
// Clap argument structures
// ---------------------------------------------------------------------------

/// Envy — encrypted environment variable manager.
///
/// Secrets are stored encrypted in a local vault (`~/.envy/vault.db`) and
/// never written to plaintext files. Use `envy run` to inject them directly
/// into your process environment.
#[derive(Parser)]
#[command(name = "envy", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialise Envy in the current directory.
    ///
    /// Creates `envy.toml` (the project manifest) and registers a new project
    /// in the vault. Must be run once per project before any other command.
    Init,

    /// Store or update a secret (KEY=VALUE).
    ///
    /// The value may contain additional `=` characters — only the first `=` is
    /// used as the key/value separator.
    Set {
        /// Secret to store in KEY=VALUE format.
        assignment: String,

        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Print the decrypted value of a secret.
    ///
    /// Outputs only the raw value — no labels or trailing metadata — so the
    /// output is safe to use in shell pipelines.
    Get {
        /// The secret key name.
        key: String,

        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// List all secret key names for the environment (never values).
    ///
    /// Keys are printed one per line in alphabetical order. Secret values are
    /// never included in the output.
    #[command(alias = "ls")]
    List {
        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Delete a secret.
    #[command(alias = "remove")]
    Rm {
        /// The secret key name to delete.
        key: String,

        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Inject secrets as environment variables and run a child process.
    ///
    /// Fetches all secrets for the selected environment, injects them into
    /// the child process environment, and proxies the child's exit code exactly.
    ///
    /// Example: envy run -e staging -- npm run dev
    Run {
        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,

        /// Command and arguments to execute (everything after `--`).
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Import secrets from a legacy `.env` file.
    ///
    /// Reads KEY=VALUE pairs line by line. Comment lines (`#`) and blank lines
    /// are skipped. Malformed lines produce a warning but do not abort the import.
    Migrate {
        /// Path to the `.env` file to import.
        file: std::path::PathBuf,

        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Vault path helper
// ---------------------------------------------------------------------------

/// Returns the path to the encrypted vault file (`~/.envy/vault.db`).
///
/// Accessible to submodules via `super::vault_path()`.
pub(super) fn vault_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".envy")
        .join("vault.db")
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Top-level entry point called by `main.rs`.
///
/// Returns the process exit code (0 = success, non-zero = failure).
/// All error printing is handled here via `eprintln!` before returning.
/// Panics are prohibited in all reachable code paths.
///
/// # Vault lifecycle
/// `Init` is the sole command that owns its own vault connection (see
/// [`commands::cmd_init`]). All other commands share a single vault opened here.
pub fn run() -> i32 {
    use clap::Parser as _;

    let cli = Cli::parse();

    // --- Init is special: it manages its own vault lifecycle. ---
    if let Commands::Init = &cli.command {
        return match commands::cmd_init() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}", format_cli_error(&e));
                cli_exit_code(&e)
            }
        };
    }

    // --- All other commands: resolve manifest, open vault once, dispatch. ---

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot determine current directory: {e}");
            return 1;
        }
    };

    let (manifest, _) = match crate::core::find_manifest(&cwd) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", format_core_error(&e));
            return core_exit_code(&e);
        }
    };

    let master_key = match crate::crypto::get_or_create_master_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            return 4;
        }
    };

    let vault = match crate::db::Vault::open(&vault_path(), master_key.as_ref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", format_cli_error(&CliError::VaultOpen(e.to_string())));
            return 4;
        }
    };

    let project_id = crate::db::ProjectId(manifest.project_id.clone());

    match cli.command {
        Commands::Init => unreachable!("Init is handled above"),

        Commands::Set { assignment, env } => {
            let env = env.as_deref().unwrap_or("");
            match assignment.split_once('=') {
                None => {
                    let e = CliError::InvalidAssignment(assignment);
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
                Some((key, value)) => {
                    match commands::cmd_set(&vault, &master_key, &project_id, env, key, value) {
                        Ok(()) => 0,
                        Err(e) => {
                            eprintln!("{}", format_core_error(&e));
                            core_exit_code(&e)
                        }
                    }
                }
            }
        }

        Commands::Get { key, env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_get(&vault, &master_key, &project_id, env, &key) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_core_error(&e));
                    core_exit_code(&e)
                }
            }
        }

        Commands::List { env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_list(&vault, &project_id, env) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_core_error(&e));
                    core_exit_code(&e)
                }
            }
        }

        Commands::Rm { key, env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_rm(&vault, &project_id, env, &key) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_core_error(&e));
                    core_exit_code(&e)
                }
            }
        }

        Commands::Run { env, command } => {
            let env = env.as_deref().unwrap_or("");
            commands::cmd_run(&vault, &master_key, &project_id, env, &command)
        }

        Commands::Migrate { file, env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_migrate(&vault, &master_key, &project_id, env, &file) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }
    }
}
