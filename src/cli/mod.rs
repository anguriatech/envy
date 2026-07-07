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
//! - `envy key export`/`import` (see [`commands::cmd_key_export`],
//!   [`commands::cmd_key_import`]) are a second, narrower exception: they
//!   manage the master key's lifecycle directly via `crate::crypto::{encode_key_hex,
//!   decode_key_hex, set_master_key}` and the existing `crate::crypto::artifact`
//!   envelope, since master-key backup/restore has no `core` representation
//!   (it isn't tied to any one project's vault).

mod commands;
mod error;
pub mod format;

use clap::{CommandFactory, Parser, Subcommand};
use format::OutputFormat;
use std::io::Read;

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

    /// Output format for read commands (default: table).
    #[arg(long, short = 'f', global = true, default_value = "table")]
    pub format: OutputFormat,
}

/// The set of subcommands recognised by the `envy` binary.
///
/// Each variant maps to one `envy <subcommand>` invocation.
#[derive(Subcommand)]
pub enum Commands {
    /// Initialise Envy in the current directory.
    ///
    /// Creates `envy.toml` (the project manifest) and registers a new project
    /// in the vault. Must be run once per project before any other command.
    Init,

    /// Store or update a secret.
    ///
    /// By default the secret is provided as KEY=VALUE.  Only the first `=`
    /// is used as the key/value separator — the value may contain additional
    /// `=` characters.
    ///
    /// With `--stdin` the VALUE is read from standard input so that it never
    /// appears in process listings (`ps`, `/proc`) or shell history:
    ///
    ///   echo "secret" | envy set --stdin KEY
    Set {
        /// Secret key name, or KEY=VALUE pair (when `--stdin` is not set).
        assignment: String,

        /// Read the secret value from stdin.  The `assignment` argument is
        /// used as the key name.
        #[arg(long)]
        stdin: bool,

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

    /// List all secret key names for the environment.
    ///
    /// Keys are printed one per line in alphabetical order.
    ///
    /// **Note**: While the default `table` format only prints key names, using
    /// `--format json`, `--format dotenv`, or `--format shell` will decrypt and
    /// reveal the actual secret values in the output.
    #[command(visible_alias = "ls")]
    List {
        /// Target environment (default: development).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Delete a secret.
    #[command(visible_aliases = ["remove", "unset"])]
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

    /// Seal the local vault into an encrypted `envy.enc` GitOps artifact.
    ///
    /// All environments are sealed by default. Use `-e` to seal a single environment.
    /// Prompts for a passphrase with confirmation, or reads `ENVY_PASSPHRASE` in CI.
    #[command(visible_alias = "enc")]
    Encrypt {
        /// Seal only this environment (default: all environments in the vault).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Unseal `envy.enc` and upsert secrets into the local vault.
    ///
    /// Successfully decrypted environments are upserted. Environments that cannot
    /// be decrypted with the provided passphrase are listed as skipped (not an error).
    /// Exits non-zero only if zero environments are imported.
    #[command(visible_alias = "dec")]
    Decrypt,

    /// Print all secrets for an environment to stdout.
    ///
    /// Default output format is `dotenv` (`KEY=value` one per line), suitable for
    /// generating `.env` files or sourcing with `eval $(envy export --format shell)`.
    /// Use `--format json` for machine-readable output.
    Export {
        /// Target environment (default: development).
        #[arg(
            short = 'e',
            long = "env",
            value_name = "ENV",
            default_value = "development"
        )]
        env: String,
    },

    /// Compare local vault secrets against the sealed envy.enc artifact.
    ///
    /// Shows additions, deletions, and modifications for one environment.
    /// By default only key names are shown — use `--reveal` to include values.
    /// Exit code 0 = no differences, 1 = differences found, 2+ = error.
    #[command(visible_alias = "df")]
    Diff {
        /// Target environment (default: development).
        #[arg(
            short = 'e',
            long = "env",
            value_name = "ENV",
            default_value = "development"
        )]
        env: String,

        /// Show decrypted secret values in the output.
        #[arg(long)]
        reveal: bool,
    },

    /// Show sync status of all vault environments.
    ///
    /// Displays a table of environments with secret count, last-modified time,
    /// and sync state relative to `envy.enc`. Read-only — never prompts for a
    /// passphrase or decrypts secret values. Use `--format json` for
    /// machine-readable output suitable for CI/CD pipelines.
    #[command(visible_alias = "st")]
    Status,

    /// Re-seal an existing envelope in `envy.enc` with a new passphrase.
    ///
    /// The current passphrase is verified before the new one is accepted,
    /// preventing the silent key-rotation that `envy encrypt` can perform
    /// in headless mode. Use this as the safe path for key rotation.
    ///
    /// Interactive mode prompts for the current, new, and confirmation
    /// passphrases. Headless mode (CI) reads `ENVY_PASSPHRASE_<ENV>` and
    /// `ENVY_PASSPHRASE_<ENV>_NEW`. Omit `-e` to select environments via
    /// a multi-select prompt.
    ///
    /// The rotation is forward-only: the old passphrase can no longer
    /// decrypt the artifact.
    Rotate {
        /// Target environment to rotate (default: MultiSelect from envy.enc).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,
    },

    /// Scan the project's working tree for plaintext copies of vault secrets.
    ///
    /// Compares every EXACT secret value already stored in the vault against
    /// the contents of tracked (and gitignore-respecting) files — this is
    /// not a generic pattern-based secret scanner. Values are masked by
    /// default; `--reveal` shows the matched value with a stderr warning.
    /// Exit code 0 = clean, 1 = leak(s) found, 2+ = error — safe to gate a
    /// pre-commit hook on.
    Scan {
        /// Restrict the scan to secrets from a single environment (default: all).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,

        /// Show the matched plaintext value in the output.
        #[arg(long)]
        reveal: bool,
    },

    /// Show the local audit trail of secret-touching actions.
    ///
    /// Records `set`, `get`, `rm`, and `run` actions (key name and timestamp
    /// only — never the secret value) in the local vault. Sync/crypto actions
    /// (`encrypt`, `decrypt`, `rotate`) are not recorded here; their history
    /// already lives in `envy.enc`'s git log and `envy status`.
    #[command(visible_alias = "au")]
    Audit {
        /// Restrict the report to a single environment (default: all).
        #[arg(short = 'e', long = "env", value_name = "ENV")]
        env: Option<String>,

        /// Maximum number of entries to show, newest first.
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },

    /// Generate shell completion scripts.
    ///
    /// Prints the completion script for the given shell to stdout.
    /// Source the output in your shell profile to enable tab-completion.
    ///
    /// Examples:
    ///   envy completions bash >> ~/.bash_completion
    ///   envy completions zsh > ~/.zfunc/_envy
    ///   envy completions fish > ~/.config/fish/completions/envy.fish
    #[command(hide = true)]
    Completions {
        /// Target shell.
        shell: clap_complete::Shell,
    },

    /// Back up or restore the local vault master key.
    ///
    /// The master key normally lives ONLY in the OS Credential Manager and is
    /// never written to disk. If it's lost (OS reinstall, keychain wiped, an
    /// ephemeral VM/container), the local vault becomes permanently
    /// unreadable — there is no other copy. `envy key export`/`import` are an
    /// opt-in, explicit way to create (and later restore from) an offline
    /// backup, protected the same way `envy.enc` is (Argon2id + AES-256-GCM).
    ///
    /// This command is independent of any single project: it operates on the
    /// one master key shared by every `envy` project on this machine, so it
    /// works even outside a directory with `envy.toml`.
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },

    /// Install a git hook that guards commits against leaked secrets.
    ///
    /// The installed `pre-commit` hook runs `envy scan` and blocks the
    /// commit if a vault secret's plaintext value is found in a staged
    /// file. It also prints a non-blocking warning if `envy status` shows
    /// unsealed drift. Nothing leaves the machine — this is pure local
    /// git tooling, no network or CI dependency required.
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
}

/// Subcommands of `envy hooks`.
#[derive(Subcommand)]
pub enum HooksAction {
    /// Install (or reinstall) the `pre-commit` hook in this project's `.git/hooks`.
    ///
    /// Refuses to overwrite a pre-existing hook that envy didn't install
    /// unless `--force` is given, in which case the existing file is backed
    /// up first (`pre-commit.envy-backup`), never silently discarded.
    Install {
        /// Overwrite an existing, non-envy pre-commit hook (after backing it up).
        #[arg(long)]
        force: bool,
    },
}

/// Subcommands of `envy key`.
#[derive(Subcommand)]
pub enum KeyAction {
    /// Write an encrypted backup of the local master key to a file.
    ///
    /// Prompts for a recovery passphrase (or reads `ENVY_KEY_RECOVERY_PASSPHRASE`
    /// for headless use). Store the resulting file somewhere OFFLINE and
    /// separate from this machine — a password manager, a safe, cold storage.
    /// Anyone with both the file and the passphrase can decrypt your vault.
    Export {
        /// Output file path (default: ./envy-key-recovery.json). Refuses to
        /// overwrite an existing file.
        #[arg(long, value_name = "FILE")]
        output: Option<std::path::PathBuf>,
    },

    /// Restore the master key from a recovery file into the OS Credential Manager.
    ///
    /// This OVERWRITES the current master key. If a local vault already
    /// exists and was encrypted under a different key, it becomes
    /// permanently unreadable unless the imported key matches. Interactive
    /// mode asks for confirmation when a local vault is present; use
    /// `--force` to skip the prompt in scripts.
    Import {
        /// Path to a file created by `envy key export`.
        input: std::path::PathBuf,

        /// Skip the confirmation prompt when a local vault already exists.
        #[arg(long)]
        force: bool,
    },
}

// ---------------------------------------------------------------------------
// Vault path helper
// ---------------------------------------------------------------------------

/// Returns the canonical path for the `envy.enc` artifact.
///
/// `envy.enc` is always co-located with `envy.toml` in the project root,
/// regardless of which subdirectory the user runs the command from.
///
/// `manifest_path` is the **directory** returned by [`crate::core::find_manifest`]
/// (not the file path itself), so joining directly produces the correct sibling path.
fn artifact_path(manifest_path: &std::path::Path) -> std::path::PathBuf {
    manifest_path.join("envy.enc")
}

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

    // --- Completions: no vault or manifest needed. ---
    if let Commands::Completions { shell } = cli.command {
        clap_complete::generate(shell, &mut Cli::command(), "envy", &mut std::io::stdout());
        return 0;
    }

    // --- Ensure ~/.envy/ exists for every command (including Init). ---
    if let Some(vault_dir) = vault_path().parent() {
        if let Err(e) = std::fs::create_dir_all(vault_dir) {
            eprintln!("error: cannot create vault directory: {e}");
            return 4;
        }
    }

    // --- Key: operates on the one machine-wide master key, independent of
    // any project manifest or vault.db — never resolve a manifest for it. ---
    if let Commands::Key { action } = cli.command {
        return match action {
            KeyAction::Export { output } => match commands::cmd_key_export(output.as_deref()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            },
            KeyAction::Import { input, force } => match commands::cmd_key_import(&input, force) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            },
        };
    }

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

    let (manifest, manifest_path) = match crate::core::find_manifest(&cwd) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", format_core_error(&e));
            return core_exit_code(&e);
        }
    };

    // --- Hooks: only needs the project root (manifest_path) — installing a
    // git hook never requires the master key or an open vault. ---
    if let Commands::Hooks { action } = cli.command {
        return match action {
            HooksAction::Install { force } => {
                match commands::cmd_hooks_install(&manifest_path, force) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", format_cli_error(&e));
                        cli_exit_code(&e)
                    }
                }
            }
        };
    }

    let master_key = match crate::crypto::get_or_create_master_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            return 4;
        }
    };

    let vp = vault_path();
    let vault = match crate::db::Vault::open(&vp, master_key.as_ref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", format_cli_error(&CliError::VaultOpen(e.to_string())));
            return 4;
        }
    };

    let project_id = crate::db::ProjectId(manifest.project_id.clone());

    // Ensure the project row exists for the UUID in envy.toml. On fresh runners the
    // vault.db is empty, so the FK on environments/secrets would fail without this.
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_owned();
    if let Err(e) = vault.ensure_project(&project_id, &project_name) {
        eprintln!("error: cannot register project in vault: {e}");
        return 4;
    }

    match cli.command {
        Commands::Init => unreachable!("Init is handled above"),

        Commands::Set {
            assignment,
            stdin,
            env,
        } => {
            let env = env.as_deref().unwrap_or("");
            if stdin {
                let mut value = String::new();
                if let Err(e) = std::io::stdin().read_to_string(&mut value) {
                    eprintln!("error: cannot read value from stdin: {e}");
                    return 4;
                }
                match commands::cmd_set(&vault, &master_key, &project_id, env, &assignment, &value)
                {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", format_core_error(&e));
                        core_exit_code(&e)
                    }
                }
            } else {
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
        }

        Commands::Get { key, env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_get(&vault, &master_key, &project_id, env, &key, cli.format) {
                Ok(()) => 0,
                Err(e) => {
                    if cli.format == OutputFormat::Table {
                        eprintln!("{}", format_cli_error(&e));
                    }
                    cli_exit_code(&e)
                }
            }
        }

        Commands::List { env } => {
            let env = env.as_deref().unwrap_or("");
            match commands::cmd_list(&vault, &master_key, &project_id, env, cli.format) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
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

        Commands::Encrypt { env } => {
            let artifact = artifact_path(&manifest_path);
            let env_filter = env.as_deref();
            match commands::cmd_encrypt(&vault, &master_key, &project_id, &artifact, env_filter) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {e}");
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Decrypt => {
            let artifact = artifact_path(&manifest_path);
            match commands::cmd_decrypt(&vault, &master_key, &project_id, &artifact) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {e}");
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Export { env } => {
            match commands::cmd_export(&vault, &master_key, &project_id, &env, cli.format) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Diff { env, reveal } => {
            let artifact = artifact_path(&manifest_path);
            match commands::cmd_diff(
                &vault,
                &master_key,
                &project_id,
                &env,
                &artifact,
                cli.format,
                reveal,
            ) {
                Ok(has_diff) => {
                    if has_diff {
                        1
                    } else {
                        0
                    }
                }
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Status => {
            let artifact = artifact_path(&manifest_path);
            match commands::cmd_status(
                &vault,
                &project_id,
                &artifact,
                cli.format,
                manifest.rotation_reminder_days,
            ) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Rotate { env } => {
            let artifact = artifact_path(&manifest_path);
            match commands::cmd_rotate(&vault, &master_key, &project_id, &artifact, env.as_deref())
            {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Scan { env, reveal } => {
            match commands::cmd_scan(
                &vault,
                &master_key,
                &project_id,
                env.as_deref(),
                &manifest_path,
                cli.format,
                reveal,
            ) {
                Ok(has_leaks) => {
                    if has_leaks {
                        1
                    } else {
                        0
                    }
                }
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Audit { env, limit } => {
            match commands::cmd_audit(&vault, &project_id, env.as_deref(), limit, cli.format) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", format_cli_error(&e));
                    cli_exit_code(&e)
                }
            }
        }

        Commands::Key { .. } => unreachable!("Key is handled above"),

        Commands::Hooks { .. } => unreachable!("Hooks is handled above"),

        Commands::Completions { .. } => unreachable!("Completions is handled above"),
    }
}
