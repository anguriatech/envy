//! Shell setup for transparent shims (`envy auto` / `envy shell-init`).
//!
//! # Background
//! `envy run -- cmd` injects secrets only into one child process. Humans and
//! AI agents keep running bare `npm run dev`, so vault secrets are silently
//! ignored. The fix that preserves envy's containment (parent shell never
//! holds secrets — see `specs/018-transparent-shims/spec.md` ADR-001) is PATH
//! shims: `~/.envy/shims/` first on `PATH`, each shim delegating to the hidden
//! `envy exec` plumbing (see `super::shim`), which injects scoped exactly
//! like `run`.
//!
//! This module only knows about shells for one purpose: printing the static
//! one-time `PATH` line (`envy shell-init`). It never touches the vault,
//! the manifest, or rc files — setup lines are printed, the user pastes them.

use std::path::Path;

// ---------------------------------------------------------------------------
// ShellKind
// ---------------------------------------------------------------------------

/// Shells supported by `envy shell-init`.
///
/// `pub` (not `pub(super)` like the rest of this module): the `Commands`
/// enum in `super` is public API, so its `ShellInit::shell` field cannot
/// expose a less-visible type (`private_interfaces` lint).
/// Re-exported from `super` for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Nushell,
}

/// Best-effort shell detection from `$SHELL` (basename match).
///
/// Falls back to [`ShellKind::Bash`]. Only used to pick which one-liner to
/// display — `shell-init` itself takes an explicit shell argument.
pub(super) fn detect_shell() -> ShellKind {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let base = shell.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    if base.contains("zsh") {
        ShellKind::Zsh
    } else if base.contains("fish") {
        ShellKind::Fish
    } else if base.contains("powershell") || base.contains("pwsh") {
        ShellKind::Powershell
    } else if base.contains("nu") {
        ShellKind::Nushell
    } else {
        ShellKind::Bash
    }
}

// ---------------------------------------------------------------------------
// shell-init snippets (static PATH lines)
// ---------------------------------------------------------------------------

/// Returns the one-time setup text for `shell` (printed by `envy shell-init`).
///
/// Static content: shims dir prepended to `PATH` plus ordering guidance.
/// Never installed automatically — the user pastes it (kept last in the rc so
/// later prepends from version managers cannot shadow the shims).
pub(super) fn shell_init_snippet(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => {
            r#"# envy shims — prefix-free runs without `envy run --`, parent stays clean.
# One-time setup: keep this line LAST in ~/.bashrc (tools like fnm/nvm prepend
# to PATH and would shadow the shims), then restart your shell.
# Verify: `which -a npm` should list $HOME/.envy/shims/npm first.
# Diagnose: `envy doctor`. Per-project opt-in: `envy auto on` + `envy reshim`.
export PATH="$HOME/.envy/shims:$PATH"
"#
        }
        ShellKind::Zsh => {
            r#"# envy shims — prefix-free runs without `envy run --`, parent stays clean.
# One-time setup: keep this line LAST in ~/.zshrc (tools like fnm/nvm prepend
# to PATH and would shadow the shims), then restart your shell.
# Verify: `which -a npm` should list $HOME/.envy/shims/npm first.
# Diagnose: `envy doctor`. Per-project opt-in: `envy auto on` + `envy reshim`.
export PATH="$HOME/.envy/shims:$PATH"
"#
        }
        ShellKind::Fish => {
            r#"# envy shims — prefix-free runs without `envy run --`, parent stays clean.
# One-time setup: keep this line near the END of ~/.config/fish/config.fish
# (later PATH prepends would shadow the shims), then restart your shell.
# Verify: `which -a npm` should list ~/.envy/shims/npm first.
# Diagnose: `envy doctor`. Per-project opt-in: `envy auto on` + `envy reshim`.
set -gx PATH "$HOME/.envy/shims" $PATH
"#
        }
        ShellKind::Powershell => {
            r#"# envy shims — prefix-free runs without `envy run --`, parent stays clean.
# One-time setup: paste this into your $PROFILE (near the end, so later PATH
# prepends don't shadow the shims), then restart your shell.
# Verify: `(Get-Command npm).Source` should be under .envy\shims.
# Diagnose: `envy doctor`. Per-project opt-in: `envy auto on` + `envy reshim`.
$env:PATH = "$HOME\.envy\shims;$env:PATH"
"#
        }
        ShellKind::Nushell => {
            r#"# envy shims — prefix-free runs without `envy run --`, parent stays clean.
# One-time setup: paste this into config.nu, then restart your shell.
# Verify: `which npm` should list the shims dir first.
# Diagnose: `envy doctor`. Per-project opt-in: `envy auto on` + `envy reshim`.
$env.PATH = ($env.PATH | prepend ($env.HOME | path join .envy shims))
"#
        }
    }
}

/// Manual install instruction for `shell` (printed when pointing the user at
/// setup — nothing is ever written automatically).
pub(super) fn oneliner_for_shell(shell: ShellKind) -> String {
    match shell {
        ShellKind::Bash => "envy shell-init bash >> ~/.bashrc".to_string(),
        ShellKind::Zsh => "envy shell-init zsh >> ~/.zshrc".to_string(),
        ShellKind::Fish => "envy shell-init fish >> ~/.config/fish/config.fish".to_string(),
        ShellKind::Powershell => {
            "paste the output of `envy shell-init powershell` into $PROFILE".to_string()
        }
        ShellKind::Nushell => {
            "paste the output of `envy shell-init nushell` into config.nu".to_string()
        }
    }
}

/// Prints the two setup steps after opting in (shared by `envy init
/// --auto-inject` and `envy auto on`). Informational only.
pub(super) fn print_setup_next_steps() {
    println!("next steps:");
    println!("  1. envy reshim");
    println!("     generate shims for this project's toolchains.");
    println!("  2. {}", oneliner_for_shell(detect_shell()));
    println!("     shims on PATH (keep the line last in your rc, then restart).");
    println!("verify anytime: envy doctor");
}

// ---------------------------------------------------------------------------
// Command handlers (pub(super) — called from cli::run)
// ---------------------------------------------------------------------------

use crate::cli::error::CliError;
use crate::db::{ProjectId, Vault};

/// `envy shell-init [SHELL]` — prints the setup snippet. No vault, no manifest.
pub(super) fn cmd_shell_init(shell: Option<ShellKind>) -> Result<(), CliError> {
    let kind = shell.unwrap_or_else(detect_shell);
    print!("{}", shell_init_snippet(kind));
    Ok(())
}

/// `envy auto [on|off|status]` — manages the per-project `auto_inject` flag.
///
/// The flag opts the project into shim injection (`envy reshim` generates the
/// shims; `envy exec` injects scoped like `run`). Never touches the shell.
pub(super) fn cmd_auto(
    vault: &Vault,
    project_id: &ProjectId,
    manifest: &crate::core::Manifest,
    manifest_dir: &Path,
    action: Option<super::AutoAction>,
    project_label: &str,
) -> Result<(), CliError> {
    use super::AutoAction;
    // Touch the vault so a missing project row surfaces the same way as
    // every other command (the row is ensured by run() beforehand).
    let _ = (vault, project_id);
    match action.unwrap_or(AutoAction::Status) {
        AutoAction::Status => {
            if manifest.auto_inject {
                println!("auto-inject: on ({project_label})");
                println!("shims: run `envy reshim` (`envy doctor` verifies setup).");
            } else {
                println!("auto-inject: off ({project_label})");
                println!("enable with: envy auto on");
            }
            Ok(())
        }
        AutoAction::On => {
            crate::core::set_manifest_auto_inject(manifest_dir, true)
                .map_err(|e| CliError::VaultOpen(e.to_string()))?;
            println!("✓ auto-inject enabled for {project_label}.");
            print_setup_next_steps();
            println!(
                "env: $ENVY_ENV or development. Disable anytime: envy auto off (or ENVY_AUTO_INJECT=0)."
            );
            Ok(())
        }
        AutoAction::Off => {
            crate::core::set_manifest_auto_inject(manifest_dir, false)
                .map_err(|e| CliError::VaultOpen(e.to_string()))?;
            println!("✓ auto-inject disabled for {project_label}.");
            println!("shims now pass through without secrets (the parent was never touched).");
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_put_shims_first_on_path() {
        let bash = shell_init_snippet(ShellKind::Bash);
        assert!(
            bash.contains("$HOME/.envy/shims:$PATH"),
            "bash must prepend the shims dir, got:\n{bash}"
        );
        let zsh = shell_init_snippet(ShellKind::Zsh);
        assert!(
            zsh.contains("$HOME/.envy/shims:$PATH"),
            "zsh must prepend the shims dir, got:\n{zsh}"
        );
        let fish = shell_init_snippet(ShellKind::Fish);
        assert!(
            fish.contains("$HOME/.envy/shims"),
            "fish must reference the shims dir, got:\n{fish}"
        );
        let ps = shell_init_snippet(ShellKind::Powershell);
        assert!(
            ps.contains(".envy\\shims"),
            "powershell must reference the shims dir, got:\n{ps}"
        );
        let nu = shell_init_snippet(ShellKind::Nushell);
        assert!(
            nu.contains(".envy"),
            "nushell must reference the shims dir, got:\n{nu}"
        );
    }

    #[test]
    fn snippets_carry_guidance_not_hooks() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
            ShellKind::Nushell,
        ] {
            let s = shell_init_snippet(shell);
            assert!(
                s.contains("envy doctor"),
                "snippet for {shell:?} must point at doctor"
            );
            assert!(
                s.contains("envy reshim"),
                "snippet for {shell:?} must point at reshim"
            );
            for forbidden in [
                "__envy_hook",
                "chpwd",
                "PROMPT_COMMAND",
                "hide-env",
                "load-env",
            ] {
                assert!(
                    !s.contains(forbidden),
                    "snippet for {shell:?} must not contain hook machinery ({forbidden})"
                );
            }
        }
    }

    #[test]
    fn oneliners_point_at_shell_init() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
            ShellKind::Nushell,
        ] {
            assert!(
                oneliner_for_shell(shell).contains("shell-init"),
                "oneliner for {shell:?}"
            );
        }
    }
}
