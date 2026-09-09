//! Transparent shell auto-injection (`envy auto` / `envy hook` / `envy shell-init`).
//!
//! # Background
//! `envy run -- cmd` injects secrets only into one child process. Humans and
//! AI agents keep running bare `npm run dev` (which reads a legacy `.env`
//! file), so vault secrets are silently ignored. This module provides the
//! standard fix used by direnv/mise: a one-time shell setup
//! (`eval "$(envy shell-init <shell>)"`) installs a `cd`/prompt hook that
//! calls `envy hook` on every navigation. `envy hook` prints `export`
//! statements for the current project to stdout (meant to be `eval`'d) and
//! warnings to stderr.
//!
//! # Safety properties
//! - `envy hook` never fails loudly: outside a project (or when `auto_inject`
//!   is off, or the vault is unreachable) it prints unload-cleanup code or
//!   nothing at all and exits 0, so a broken vault can never break the prompt.
//! - Key names from the vault AND from the attacker-controllable `__ENVY_KEYS`
//!   tracking variable are validated with [`is_valid_shell_key`] before being
//!   interpolated — no shell-code injection via crafted key names.
//! - Values are single-quote escaped per shell (POSIX / fish / PowerShell).
//! - Auto-injection exports into the interactive shell (visible to every child
//!   process). Unlike the scoped `envy run`, this is a deliberate tradeoff,
//!   documented in every snippet header: prefer `envy run` in CI/production.
//!
//! # Precedence
//! Exported vault variables live in the parent environment, and every major
//! framework's dotenv loader (Node `dotenv`, Vite, Next.js, Django, Rails)
//! refuses to overwrite already-set variables — so envy wins over a legacy
//! `.env` file by construction. When both exist, a warning goes to stderr.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// ShellKind
// ---------------------------------------------------------------------------

/// Shells supported by `envy shell-init` / `envy hook --shell`.
///
/// `pub` (not `pub(super)` like the rest of this module): the `Commands`
/// enum in `super` is public API, so its `ShellInit::shell` / `Hook::shell`
/// fields cannot expose a less-visible type (`private_interfaces` lint).
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

impl ShellKind {
    fn name(self) -> &'static str {
        match self {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
            ShellKind::Fish => "fish",
            ShellKind::Powershell => "powershell",
            ShellKind::Nushell => "nushell",
        }
    }
}

/// Best-effort shell detection from `$SHELL` (basename match).
///
/// Falls back to [`ShellKind::Bash`]. Note this reflects the login shell, not
/// necessarily the running one — which is why every `shell-init` snippet
/// passes an explicit `--shell` to `envy hook`.
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
// Env-var helpers
// ---------------------------------------------------------------------------

/// Tracking variable holding the colon-joined keys envy exported last time.
///
/// The shell snippet `eval`s hook output, so this variable round-trips
/// parent → child on every invocation and lets the hook compute which stale
/// keys to `unset` (project switch, secret deletion, leaving the tree).
pub(super) const TRACKING_VAR: &str = "__ENVY_KEYS";
const TRACKING_ENV_VAR: &str = "__ENVY_ENV";

/// Global kill-switch / force-enable for auto-injection.
///
/// - `ENVY_AUTO_INJECT=0|false|no` → hook only unloads, never injects.
/// - `ENVY_AUTO_INJECT=1|true|yes` → hook injects even if the manifest flag is
///   off (escape hatch for testing / global opt-in).
/// - Unset or anything else → respect the per-project `auto_inject` flag.
fn global_switch() -> Option<bool> {
    match std::env::var("ENVY_AUTO_INJECT")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "0" | "false" | "no" | "off" => Some(false),
        "1" | "true" | "yes" | "on" => Some(true),
        _ => None,
    }
}

/// Resolves the target environment for a hook: `--env` flag > `ENVY_ENV` >
/// `development`, normalised exactly like core (empty → default, lowercase).
pub(super) fn resolve_hook_env(flag: Option<&str>) -> String {
    let raw = match flag {
        Some(f) if !f.trim().is_empty() => f.trim().to_string(),
        _ => std::env::var("ENVY_ENV").unwrap_or_default(),
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        crate::core::DEFAULT_ENV.to_string()
    } else {
        trimmed.to_lowercase()
    }
}

/// Previously exported keys, from the parent shell via `$__ENVY_KEYS`.
pub(super) fn prev_exported_keys() -> Vec<String> {
    let raw = std::env::var(TRACKING_VAR).unwrap_or_default();
    let mut out = Vec::new();
    for part in raw.split(':') {
        let k = part.trim();
        if !k.is_empty() && !out.iter().any(|e: &String| e.as_str() == k) {
            out.push(k.to_string());
        }
    }
    out
}

/// Returns `true` iff `key` is safe to interpolate as a shell variable name.
pub(super) fn is_valid_shell_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// HookPlan — pure, testable core of `envy hook`
// ---------------------------------------------------------------------------

/// What `envy hook` must emit for one invocation (no I/O).
pub(super) struct HookPlan {
    /// Normalised environment name that was injected (empty when unloading).
    pub env_name: String,
    /// Valid `(key, value)` pairs to export, sorted by key.
    pub secrets: Vec<(String, String)>,
    /// Vault keys dropped because they are not valid shell identifiers.
    pub skipped_keys: Vec<String>,
    /// Previously exported keys that must be unset (validated, sorted).
    pub unset: Vec<String>,
    /// Currently exported keys, for the `$__ENVY_KEYS` tracking var.
    pub keys: Vec<String>,
    /// Whether a legacy `.env` sits next to the manifest (warn on stderr).
    pub has_dotenv: bool,
    /// Manifest directory that provided the secrets (for nushell JSON).
    pub project_dir: PathBuf,
}

/// Computes the export/unload sets from vault secrets + previous tracking.
///
/// - `vault_secrets`: all `(key, value)` pairs decrypted for the env.
/// - `prev_keys`: raw `$__ENVY_KEYS` contents (validated here — the parent
///   shell environment is attacker-influenced and must never reach `eval`
///   unfiltered).
pub(super) fn compute_hook_plan(
    mut vault_secrets: Vec<(String, String)>,
    prev_keys: &[String],
    env_name: &str,
    has_dotenv: bool,
    project_dir: &Path,
) -> HookPlan {
    vault_secrets.sort_by(|a, b| a.0.cmp(&b.0));
    let mut secrets = Vec::new();
    let mut skipped_keys = Vec::new();
    for (k, v) in vault_secrets {
        if is_valid_shell_key(&k) {
            secrets.push((k, v));
        } else {
            skipped_keys.push(k);
        }
    }
    let current: std::collections::BTreeSet<&str> =
        secrets.iter().map(|(k, _)| k.as_str()).collect();
    let mut unset: Vec<String> = prev_keys
        .iter()
        .filter(|k| is_valid_shell_key(k) && !current.contains(k.as_str()))
        .cloned()
        .collect();
    unset.sort();
    unset.dedup();
    let keys: Vec<String> = secrets.iter().map(|(k, _)| k.clone()).collect();
    HookPlan {
        env_name: env_name.to_string(),
        secrets,
        skipped_keys,
        unset,
        keys,
        has_dotenv,
        project_dir: project_dir.to_path_buf(),
    }
}

/// Unload-only plan: no project in scope — drop every previously exported key.
pub(super) fn compute_unload_plan(prev_keys: &[String]) -> Vec<String> {
    let mut unset: Vec<String> = prev_keys
        .iter()
        .filter(|k| is_valid_shell_key(k))
        .cloned()
        .collect();
    unset.sort();
    unset.dedup();
    unset
}

/// Returns `true` when the injected key set differs from the previously
/// exported one — i.e. the shell just entered the project, switched
/// environment, or the vault contents changed.
///
/// `plan_keys` comes from [`compute_hook_plan`] (sorted); `prev_keys` is the
/// raw `$__ENVY_KEYS` content, validated and sorted here so a poisoned
/// tracking variable can neither suppress nor force warnings.
///
/// Entry warnings (legacy `.env` precedence, skipped keys) are gated on this:
/// both `chpwd` and `precmd` invoke the hook, so an unconditional warning
/// would print twice per `cd` and spam every Enter afterwards.
pub(super) fn hook_state_changed(plan_keys: &[String], prev_keys: &[String]) -> bool {
    let mut prev_valid: Vec<String> = prev_keys
        .iter()
        .filter(|k| is_valid_shell_key(k))
        .cloned()
        .collect();
    prev_valid.sort();
    prev_valid.dedup();
    let mut current: Vec<String> = plan_keys.to_vec();
    current.sort();
    current.dedup();
    prev_valid != current
}

// ---------------------------------------------------------------------------
// Escaping + rendering (stdout payload only — warnings go to stderr)
// ---------------------------------------------------------------------------

fn escape_posix(value: &str) -> String {
    value.replace('\'', r"'\''")
}

fn escape_fish(value: &str) -> String {
    value.replace('\\', r"\\").replace('\'', r"\'")
}

fn escape_powershell(value: &str) -> String {
    value.replace('\'', "''")
}

fn join_keys(keys: &[String]) -> String {
    keys.join(":")
}

/// Renders a [`HookPlan`] as shell code (or JSON for nushell).
pub(super) fn render_hook_plan(shell: ShellKind, plan: &HookPlan) -> String {
    match shell {
        ShellKind::Bash | ShellKind::Zsh => {
            let mut out = String::new();
            for k in &plan.unset {
                out.push_str(&format!("unset {k};\n"));
            }
            for (k, v) in &plan.secrets {
                out.push_str(&format!("export {k}='{}';\n", escape_posix(v)));
            }
            if plan.keys.is_empty() {
                out.push_str(&format!("unset {TRACKING_VAR};\n"));
            } else {
                out.push_str(&format!(
                    "export {TRACKING_VAR}='{}';\n",
                    join_keys(&plan.keys)
                ));
            }
            if plan.env_name.is_empty() {
                out.push_str(&format!("unset {TRACKING_ENV_VAR};\n"));
            } else {
                out.push_str(&format!("export {TRACKING_ENV_VAR}='{}';\n", plan.env_name));
            }
            out
        }
        ShellKind::Fish => {
            let mut out = String::new();
            for k in &plan.unset {
                out.push_str(&format!("set -e {k};\n"));
            }
            for (k, v) in &plan.secrets {
                out.push_str(&format!("set -gx {k} '{}';\n", escape_fish(v)));
            }
            if plan.keys.is_empty() {
                out.push_str(&format!("set -e {TRACKING_VAR};\n"));
            } else {
                out.push_str(&format!(
                    "set -gx {TRACKING_VAR} '{}';\n",
                    join_keys(&plan.keys)
                ));
            }
            if plan.env_name.is_empty() {
                out.push_str(&format!("set -e {TRACKING_ENV_VAR};\n"));
            } else {
                out.push_str(&format!(
                    "set -gx {TRACKING_ENV_VAR} '{}';\n",
                    plan.env_name
                ));
            }
            out
        }
        ShellKind::Powershell => {
            let mut out = String::new();
            for k in &plan.unset {
                out.push_str(&format!(
                    "Remove-Item Env:\\{k} -ErrorAction SilentlyContinue;\n"
                ));
            }
            for (k, v) in &plan.secrets {
                out.push_str(&format!("$env:{k}='{}';\n", escape_powershell(v)));
            }
            if plan.keys.is_empty() {
                out.push_str(&format!(
                    "Remove-Item Env:\\{TRACKING_VAR} -ErrorAction SilentlyContinue;\n"
                ));
            } else {
                out.push_str(&format!(
                    "$env:{TRACKING_VAR}='{}';\n",
                    join_keys(&plan.keys)
                ));
            }
            if plan.env_name.is_empty() {
                out.push_str(&format!(
                    "Remove-Item Env:\\{TRACKING_ENV_VAR} -ErrorAction SilentlyContinue;\n"
                ));
            } else {
                out.push_str(&format!("$env:{TRACKING_ENV_VAR}='{}';\n", plan.env_name));
            }
            out
        }
        ShellKind::Nushell => {
            // Machine-readable: the nushell snippet applies it via load-env.
            let mut secrets = serde_json::Map::new();
            for (k, v) in &plan.secrets {
                secrets.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            let payload = serde_json::json!({
                "secrets": secrets,
                "unset": plan.unset,
                "keys": plan.keys,
                "env": plan.env_name,
                "project_dir": plan.project_dir.display().to_string(),
            });
            format!("{payload}\n")
        }
    }
}

/// Renders an unload-only payload (no project / auto-inject off / killed).
///
/// Empty stdout when there is genuinely nothing to unload, so the common
/// case (prompt outside any envy project) costs the shell a single empty
/// `eval`. Tracking vars are cleared alongside the keys whenever set.
pub(super) fn render_unload(shell: ShellKind, prev_keys: &[String]) -> String {
    let unset = compute_unload_plan(prev_keys);
    let tracking_set = std::env::var(TRACKING_VAR)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var(TRACKING_ENV_VAR)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    if unset.is_empty() && !tracking_set {
        return String::new();
    }
    let plan = HookPlan {
        env_name: String::new(),
        secrets: Vec::new(),
        skipped_keys: Vec::new(),
        unset,
        keys: Vec::new(),
        has_dotenv: false,
        project_dir: PathBuf::new(),
    };
    render_hook_plan(shell, &plan)
}

// ---------------------------------------------------------------------------
// shell-init snippets
// ---------------------------------------------------------------------------

const SECURITY_NOTE: &str = "Security note: unlike `envy run` (scoped to one child process), auto-inject exports secrets into your interactive shell, visible to every child process. Prefer `envy run` in CI and for production deploys.";

/// Returns the `eval`-able snippet for `shell` (printed by `envy shell-init`).
pub(super) fn shell_init_snippet(shell: ShellKind) -> String {
    match shell {
        ShellKind::Bash => format!(
            r#"# >>> envy auto-inject (bash) >>>
# One-time setup: add `eval "$(envy shell-init bash)"` to ~/.bashrc, then restart the shell.
# Entering a directory whose envy.toml has `auto_inject = true` (see `envy auto on`)
# auto-exports its vault secrets; leaving unloads them. Vault values win over a
# legacy `.env` file (warning on stderr). Environment: $ENVY_ENV or development.
# Escape hatch: ENVY_AUTO_INJECT=0 disables globally. For direnv users, put
# `eval "$(envy hook --shell bash)"` in the project's .envrc instead, then `direnv allow`.
# {SECURITY_NOTE}
__envy_hook() {{
    if ! command -v envy >/dev/null 2>&1; then return 0; fi
    local _envy_out
    _envy_out="$(envy hook --shell bash)" || return 0
    [ -n "$_envy_out" ] && eval "$_envy_out"
}}
case "${{PROMPT_COMMAND:-}}" in
    *__envy_hook*) ;;
    "") PROMPT_COMMAND="__envy_hook" ;;
    *) PROMPT_COMMAND="__envy_hook;${{PROMPT_COMMAND}}" ;;
esac
# <<< envy auto-inject <<<
"#
        ),
        ShellKind::Zsh => format!(
            r#"# >>> envy auto-inject (zsh) >>>
# One-time setup: add `eval "$(envy shell-init zsh)"` to ~/.zshrc, then restart the shell.
# Entering a directory whose envy.toml has `auto_inject = true` (see `envy auto on`)
# auto-exports its vault secrets; leaving unloads them. Vault values win over a
# legacy `.env` file (warning on stderr). Environment: $ENVY_ENV or development.
# Escape hatch: ENVY_AUTO_INJECT=0 disables globally. For direnv users, put
# `eval "$(envy hook --shell zsh)"` in the project's .envrc instead, then `direnv allow`.
# {SECURITY_NOTE}
__envy_hook() {{
    if ! command -v envy >/dev/null 2>&1; then return 0; fi
    local _envy_out
    _envy_out="$(envy hook --shell zsh)" || return 0
    [ -n "$_envy_out" ] && eval "$_envy_out"
}}
if [[ -z "${{chpwd_functions[(I)__envy_hook]:-}}" ]]; then chpwd_functions+=(__envy_hook); fi
if [[ -z "${{precmd_functions[(I)__envy_hook]:-}}" ]]; then precmd_functions+=(__envy_hook); fi
# <<< envy auto-inject <<<
"#
        ),
        ShellKind::Fish => format!(
            r#"# >>> envy auto-inject (fish) >>>
# One-time setup: `envy shell-init fish >> ~/.config/fish/config.fish`, then restart the shell.
# Entering a directory whose envy.toml has `auto_inject = true` (see `envy auto on`)
# auto-exports its vault secrets; leaving unloads them. Vault values win over a
# legacy `.env` file (warning on stderr). Environment: $ENVY_ENV or development.
# Escape hatch: `set -gx ENVY_AUTO_INJECT 0` disables globally.
# {SECURITY_NOTE}
function __envy_hook --on-variable PWD --description 'envy auto-inject exports vault secrets'
    if not command -q envy; return 0; end
    envy hook --shell fish | source
end
__envy_hook
# <<< envy auto-inject <<<
"#
        ),
        ShellKind::Powershell => format!(
            r#"# >>> envy auto-inject (powershell) >>>
# One-time setup: add `Invoke-Expression (& envy shell-init powershell | Out-String)` to $PROFILE, then restart the shell.
# Entering a directory whose envy.toml has `auto_inject = true` (see `envy auto on`)
# auto-exports its vault secrets; leaving unloads them. Vault values win over a
# legacy `.env` file (warning on stderr). Environment: $env:ENVY_ENV or development.
# Escape hatch: $env:ENVY_AUTO_INJECT = "0" disables globally.
# {SECURITY_NOTE}
function global:__envy_hook {{
    if (-not (Get-Command envy -ErrorAction SilentlyContinue)) {{ return }}
    $out = & envy hook --shell powershell | Out-String
    if ($out -and $out.Trim()) {{ Invoke-Expression $out }}
}}
if (-not (Test-Path Function:\global:__envy_orig_prompt)) {{
    if (Test-Path Function:\prompt) {{ Copy-Item Function:\prompt Function:\global:__envy_orig_prompt }}
    else {{ function global:__envy_orig_prompt {{ "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) " }} }}
}}
function global:prompt {{
    global:__envy_hook
    global:__envy_orig_prompt
}}
# <<< envy auto-inject <<<
"#
        ),
        ShellKind::Nushell => format!(
            r#"# >>> envy auto-inject (nushell) >>>
# One-time setup: paste this into config.nu (`$nu.config-path`), then restart the shell.
# Entering a directory whose envy.toml has `auto_inject = true` (see `envy auto on`)
# auto-exports its vault secrets; leaving unloads them. Vault values win over a
# legacy `.env` file (warning on stderr). Environment: $env.ENVY_ENV or development.
# Escape hatch: $env.ENVY_AUTO_INJECT = "0" disables globally.
# {SECURITY_NOTE}
def --env envy-hook [] {{
    let res = (do {{ envy hook --shell nushell }} | complete)
    if $res.exit_code != 0 {{ return }}
    let trimmed = ($res.stdout | str trim)
    if ($trimmed | is-empty) {{ return }}
    let data = ($trimmed | from json)
    for k in ($data.unset? | default []) {{ try {{ hide-env $k }} }}
    if (($data.secrets? | default {{}} | columns | length) > 0) {{ load-env $data.secrets }}
    if (($data.keys? | default [] | length) > 0) {{
        $env.__ENVY_KEYS = ($data.keys | str join ":")
    }} else {{
        try {{ hide-env __ENVY_KEYS }}
    }}
    if (($data.env? | default "" | str trim | is-not-empty)) {{
        $env.__ENVY_ENV = $data.env
    }} else {{
        try {{ hide-env __ENVY_ENV }}
    }}
}}
# Merge with your existing hooks (keeps what you already have):
# $env.config.hooks.env_change.PWD = ($env.config.hooks.env_change.PWD? | default [] | append {{|before, after| envy-hook }})
# <<< envy auto-inject <<<
"#
        ),
    }
}

// ---------------------------------------------------------------------------
// One-step hook installation (`envy init --auto-inject`)
// ---------------------------------------------------------------------------

/// Opening marker line of every snippet from [`shell_init_snippet`].
///
/// Used to detect an existing installation before appending (idempotency).
pub(super) fn snippet_marker(shell: ShellKind) -> String {
    format!("# >>> envy auto-inject ({}) >>>", shell.name())
}

/// Returns `true` when `rc_content` already installs the hook for `shell`.
///
/// Matches the full snippet (via [`snippet_marker`]) as well as a hand-added
/// line from the docs (`envy hook --shell <shell>` / `envy shell-init
/// <shell>`), so re-running the installer never stacks a duplicate hook on
/// top of a manual setup. Matching is shell-specific: a bash snippet does
/// not count as a zsh installation.
pub(super) fn hook_already_installed(rc_content: &str, shell: ShellKind) -> bool {
    let marker = snippet_marker(shell);
    let hook_ref = format!("envy hook --shell {}", shell.name());
    let init_ref = format!("envy shell-init {}", shell.name());
    rc_content.contains(marker.as_str())
        || rc_content.contains(hook_ref.as_str())
        || rc_content.contains(init_ref.as_str())
}

/// Deterministic rc file for shells with a conventional location.
///
/// `bash` → `~/.bashrc`, `zsh` → `~/.zshrc`, `fish` →
/// `~/.config/fish/config.fish`. Returns `None` for `powershell`/`nushell`
/// (profile paths vary per platform) and when the home directory cannot be
/// resolved — those cases fall back to printed manual instructions.
pub(super) fn rc_file_for_shell(shell: ShellKind) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let rel: &str = match shell {
        ShellKind::Bash => ".bashrc",
        ShellKind::Zsh => ".zshrc",
        ShellKind::Fish => ".config/fish/config.fish",
        ShellKind::Powershell | ShellKind::Nushell => return None,
    };
    Some(home.join(rel))
}

/// One-line manual setup for `shell` (used when auto-install is declined,
/// impossible, or the shell has no deterministic rc file).
pub(super) fn oneliner_for_shell(shell: ShellKind) -> String {
    match shell {
        ShellKind::Bash => "envy shell-init bash >> ~/.bashrc".to_string(),
        ShellKind::Zsh => "envy shell-init zsh >> ~/.zshrc".to_string(),
        ShellKind::Fish => "envy shell-init fish >> ~/.config/fish/config.fish".to_string(),
        ShellKind::Powershell => {
            "add `Invoke-Expression (& envy shell-init powershell | Out-String)` to $PROFILE"
                .to_string()
        }
        ShellKind::Nushell => {
            "paste `envy shell-init nushell` into config.nu ($nu.config-path)".to_string()
        }
    }
}

/// Appends the full [`shell_init_snippet`] to `rc_path` unless already present.
///
/// Creates missing parent directories (e.g. `~/.config/fish`). Returns
/// `Ok(true)` when the snippet was appended, `Ok(false)` when the hook was
/// already installed — never duplicates. Existing file content is preserved
/// byte-for-byte (only a missing trailing newline is added first).
pub(super) fn append_snippet_if_missing(rc_path: &Path, shell: ShellKind) -> Result<bool, String> {
    let existing = match std::fs::read_to_string(rc_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    if hook_already_installed(&existing, shell) {
        return Ok(false);
    }
    if let Some(parent) = rc_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_path)
        .map_err(|e| e.to_string())?;
    {
        use std::io::Write as _;
        if !existing.is_empty() && !existing.ends_with('\n') {
            file.write_all(b"\n").map_err(|e| e.to_string())?;
        }
        file.write_all(shell_init_snippet(shell).as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// Offers to install the shell hook during `envy init --auto-inject`.
///
/// Best-effort and infallible by design: the project is already initialised
/// when this runs, so every failure (unwritable rc file, declined prompt,
/// no TTY, exotic shell) degrades to printed manual instructions — init
/// itself is never failed retroactively.
pub(super) fn offer_hook_install() {
    let shell = detect_shell();
    let Some(rc_path) = rc_file_for_shell(shell) else {
        println!(
            "one-time shell setup ({} needs one manual step):",
            shell.name()
        );
        println!("  {}", oneliner_for_shell(shell));
        return;
    };
    let already = std::fs::read_to_string(&rc_path)
        .map(|c| hook_already_installed(&c, shell))
        .unwrap_or(false);
    if already {
        println!(
            "shell hook already present in {} — nothing to do.",
            rc_path.display()
        );
        return;
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        println!("to finish setup, run:");
        println!("  {}", oneliner_for_shell(shell));
        println!("(then restart your shell)");
        return;
    }
    let prompt = format!(
        "Detected shell: {} — append the envy auto-inject hook to {}?",
        shell.name(),
        rc_path.display()
    );
    let confirmed = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false);
    if !confirmed {
        println!("skipped shell setup. To enable later, run:");
        println!("  {}", oneliner_for_shell(shell));
        return;
    }
    match append_snippet_if_missing(&rc_path, shell) {
        Ok(true) => println!(
            "✓ shell hook installed in {} (restart your shell to activate).",
            rc_path.display()
        ),
        Ok(false) => println!(
            "shell hook already present in {} — nothing to do.",
            rc_path.display()
        ),
        Err(e) => {
            eprintln!("warning: could not update {}: {e}", rc_path.display());
            println!("to finish setup manually, run:");
            println!("  {}", oneliner_for_shell(shell));
        }
    }
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
                println!(
                    "shell setup: eval \"$(envy shell-init <bash|zsh|fish|powershell|nushell>)\""
                );
            } else {
                println!("auto-inject: off ({project_label})");
                println!(
                    "enable with: envy auto on   (then complete the one-time shell setup it prints)"
                );
            }
            Ok(())
        }
        AutoAction::On => {
            crate::core::set_manifest_auto_inject(manifest_dir, true)
                .map_err(|e| CliError::VaultOpen(e.to_string()))?;
            println!("✓ auto-inject enabled for {project_label}.");
            // Same one-step offer as `envy init --auto-inject`: best-effort,
            // never fails the command (declined prompt → manual instructions).
            offer_hook_install();
            println!(
                "env: $ENVY_ENV or development. Disable anytime: envy auto off (or ENVY_AUTO_INJECT=0)."
            );
            println!(
                "other shells: `envy shell-init <bash|zsh|fish|powershell|nushell>`; direnv: `eval \"$(envy hook --shell bash)\"` in .envrc."
            );
            Ok(())
        }
        AutoAction::Off => {
            crate::core::set_manifest_auto_inject(manifest_dir, false)
                .map_err(|e| CliError::VaultOpen(e.to_string()))?;
            println!("✓ auto-inject disabled for {project_label}.");
            println!(
                "run `envy hook --shell <shell>` output once more (or cd out and back) to unload already-exported keys."
            );
            Ok(())
        }
    }
}

/// `envy hook` — prints `eval`-able exports for the current directory.
///
/// This function owns its exit-code contract directly: it ALWAYS succeeds
/// (exit 0). A prompt hook must never break the shell, so every failure mode
/// — no manifest, flag off, missing vault, locked keyring, unknown env —
/// degrades to unload-cleanup or silent no-op.
pub(super) fn cmd_hook(shell: Option<ShellKind>, env_flag: Option<&str>) -> i32 {
    let kind = shell.unwrap_or(ShellKind::Bash);
    // Empty stdout = nothing to eval. stdout carries code, stderr warnings.
    let emit = |text: &str| {
        use std::io::Write as _;
        let _ = std::io::stdout().write_all(text.as_bytes());
    };

    let prev = prev_exported_keys();

    // Global kill-switch: unload, never inject.
    if global_switch() == Some(false) {
        emit(&render_unload(kind, &prev));
        return 0;
    }
    let forced = global_switch() == Some(true);

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let (manifest, manifest_dir) = match crate::core::find_manifest(&cwd) {
        Ok(m) => m,
        Err(_) => {
            emit(&render_unload(kind, &prev));
            return 0;
        }
    };
    if !manifest.auto_inject && !forced {
        emit(&render_unload(kind, &prev));
        return 0;
    }

    let env_name = resolve_hook_env(env_flag);
    let has_dotenv = manifest_dir.join(".env").is_file();

    // Vault access must never break the prompt: any failure is a silent no-op
    // (keep the current shell env untouched — it may still hold good values).
    let vault_path = super::vault_path();
    let master_key = match crate::crypto::get_or_create_master_key() {
        Ok(k) => k,
        Err(_) => return 0,
    };
    let vault = match Vault::open(&vault_path, master_key.as_ref()) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let project_id = ProjectId(manifest.project_id);
    // `get_env_secrets` returns an empty map for a missing environment or a
    // project row that does not exist yet (fresh machine, vault moved) — both
    // correctly degrade to "unload stale keys". Only hard DB/crypto failures
    // are silent no-ops that keep the current shell env untouched.
    let secrets_map =
        match crate::core::get_env_secrets(&vault, &master_key, &project_id, &env_name) {
            Ok(m) => m,
            Err(_) => return 0,
        };
    let pairs: Vec<(String, String)> = secrets_map
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect();

    let plan = compute_hook_plan(pairs, &prev, &env_name, has_dotenv, &manifest_dir);
    // Entry warnings fire once per transition (see `hook_state_changed`).
    if hook_state_changed(&plan.keys, &prev) {
        if plan.has_dotenv && !plan.secrets.is_empty() {
            eprintln!(
                "envy: warning: .env found in {} — envy vars take precedence (auto-inject, env '{}')",
                manifest_dir.display(),
                plan.env_name
            );
        }
        for skipped in &plan.skipped_keys {
            eprintln!("envy: warning: skipping key '{skipped}': not a valid shell variable name");
        }
    }
    emit(&render_hook_plan(kind, &plan));
    0
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_shell_keys() {
        assert!(is_valid_shell_key("FOO"));
        assert!(is_valid_shell_key("_x1"));
        assert!(is_valid_shell_key("aBc_123"));
        assert!(!is_valid_shell_key(""));
        assert!(!is_valid_shell_key("1ABC"));
        assert!(!is_valid_shell_key("FOO-BAR"));
        assert!(!is_valid_shell_key("FOO BAR"));
        assert!(!is_valid_shell_key("FOO=BAR"));
        assert!(!is_valid_shell_key("FOO;rm -rf"));
        assert!(!is_valid_shell_key("$FOO"));
    }

    #[test]
    fn plan_unsets_stale_and_exports_current_sorted() {
        let plan = compute_hook_plan(
            vec![
                ("B".to_string(), "2".to_string()),
                ("A".to_string(), "1".to_string()),
            ],
            &["OLD".to_string(), "B".to_string()],
            "development",
            false,
            Path::new("/proj"),
        );
        assert_eq!(plan.unset, vec!["OLD".to_string()]);
        assert_eq!(plan.keys, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(plan.secrets[0].0, "A");
    }

    #[test]
    fn plan_rejects_poisoned_prev_keys() {
        let plan = compute_hook_plan(
            vec![],
            &["FOO;evil".to_string(), "OK".to_string()],
            "development",
            false,
            Path::new("/proj"),
        );
        assert_eq!(plan.unset, vec!["OK".to_string()]);
        let out = render_hook_plan(ShellKind::Bash, &plan);
        assert!(!out.contains("evil"));
    }

    #[test]
    fn plan_skips_invalid_vault_keys_with_warning_list() {
        let plan = compute_hook_plan(
            vec![
                ("GOOD".to_string(), "1".to_string()),
                ("BAD-KEY".to_string(), "2".to_string()),
            ],
            &[],
            "development",
            false,
            Path::new("/proj"),
        );
        assert_eq!(plan.skipped_keys, vec!["BAD-KEY".to_string()]);
        let out = render_hook_plan(ShellKind::Bash, &plan);
        assert!(out.contains("GOOD"));
        assert!(!out.contains("BAD-KEY"));
    }

    #[test]
    fn bash_render_escapes_single_quotes() {
        let plan = compute_hook_plan(
            vec![("MSG".to_string(), "it's".to_string())],
            &[],
            "development",
            false,
            Path::new("/proj"),
        );
        let out = render_hook_plan(ShellKind::Bash, &plan);
        assert!(out.contains("export MSG='it'\\''s';"));
        assert!(out.contains("export __ENVY_KEYS='MSG';"));
    }

    #[test]
    fn unload_render_clears_tracking() {
        let out = render_hook_plan(
            ShellKind::Bash,
            &HookPlan {
                env_name: String::new(),
                secrets: vec![],
                skipped_keys: vec![],
                unset: vec!["A".to_string()],
                keys: vec![],
                has_dotenv: false,
                project_dir: PathBuf::new(),
            },
        );
        assert!(out.contains("unset A;"));
        assert!(out.contains("unset __ENVY_KEYS;"));
    }

    #[test]
    fn fish_and_powershell_render() {
        let plan = compute_hook_plan(
            vec![("A".to_string(), "1".to_string())],
            &["OLD".to_string()],
            "development",
            false,
            Path::new("/proj"),
        );
        let fish = render_hook_plan(ShellKind::Fish, &plan);
        assert!(fish.contains("set -e OLD;"));
        assert!(fish.contains("set -gx A '1';"));
        let ps = render_hook_plan(ShellKind::Powershell, &plan);
        assert!(ps.contains("Remove-Item Env:\\OLD"));
        assert!(ps.contains("$env:A='1';"));
    }

    #[test]
    fn nushell_renders_json() {
        let plan = compute_hook_plan(
            vec![("A".to_string(), "1".to_string())],
            &["OLD".to_string()],
            "staging",
            false,
            Path::new("/proj"),
        );
        let out = render_hook_plan(ShellKind::Nushell, &plan);
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
        assert_eq!(v["secrets"]["A"], "1");
        assert_eq!(v["unset"][0], "OLD");
        assert_eq!(v["env"], "staging");
    }

    #[test]
    fn hook_env_resolution_order() {
        //(flag, ENVY_ENV, expected)
        let _lock = ENV_GUARD.lock().unwrap();
        unsafe { std::env::remove_var("ENVY_ENV") };
        assert_eq!(resolve_hook_env(None), "development");
        assert_eq!(resolve_hook_env(Some("Staging")), "staging");
        unsafe { std::env::set_var("ENVY_ENV", "Production") };
        assert_eq!(resolve_hook_env(None), "production");
        assert_eq!(resolve_hook_env(Some("dev")), "dev");
        unsafe { std::env::remove_var("ENVY_ENV") };
    }

    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn snippets_mention_hook_and_security() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
            ShellKind::Nushell,
        ] {
            let s = shell_init_snippet(shell);
            assert!(s.contains("envy hook"), "snippet for {}", shell.name());
            assert!(
                s.contains("ENVY_AUTO_INJECT"),
                "snippet for {}",
                shell.name()
            );
        }
    }

    #[test]
    fn manifest_auto_inject_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        crate::core::create_manifest_with_options(tmp.path(), "test-id", true)
            .expect("create with auto_inject");
        let (m, _) = crate::core::find_manifest(tmp.path()).expect("find");
        assert!(m.auto_inject);
        crate::core::set_manifest_auto_inject(tmp.path(), false).expect("disable");
        let (m2, _) = crate::core::find_manifest(tmp.path()).expect("find again");
        assert!(!m2.auto_inject);
        // Legacy file without the flag parses as false.
        std::fs::write(tmp.path().join("envy.toml"), "project_id = \"x\"\n").expect("write");
        let (m3, _) = crate::core::find_manifest(tmp.path()).expect("find legacy");
        assert!(!m3.auto_inject);
    }

    #[test]
    fn install_detection_covers_snippet_and_hand_added_hook() {
        assert!(!hook_already_installed("", ShellKind::Zsh));
        assert!(!hook_already_installed(
            "export PATH=$PATH:/usr/local/bin\n",
            ShellKind::Zsh
        ));
        // Full snippet (as installed by append_snippet_if_missing).
        let snippet = shell_init_snippet(ShellKind::Zsh);
        assert!(hook_already_installed(&snippet, ShellKind::Zsh));
        // Hand-added eval line from the docs (no marker) must also count.
        assert!(hook_already_installed(
            "eval \"$(envy shell-init zsh)\"\n",
            ShellKind::Zsh
        ));
        // A snippet for another shell does not count.
        assert!(!hook_already_installed(
            &shell_init_snippet(ShellKind::Bash),
            ShellKind::Zsh
        ));
    }

    #[test]
    fn append_is_idempotent_and_preserves_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rc = tmp.path().join(".zshrc");
        std::fs::write(&rc, "export PATH=$PATH:/x\nno-trailing-newline").expect("seed rc");

        assert!(
            append_snippet_if_missing(&rc, ShellKind::Zsh).expect("append must not fail"),
            "first install must append"
        );
        let content = std::fs::read_to_string(&rc).expect("read rc");
        let marker = snippet_marker(ShellKind::Zsh);
        assert!(
            content.starts_with("export PATH=$PATH:/x\nno-trailing-newline\n"),
            "original content must be preserved with newline fix, got:\n{content}"
        );
        assert!(
            content.contains(marker.as_str()),
            "snippet marker must be present"
        );

        assert!(
            !append_snippet_if_missing(&rc, ShellKind::Zsh).expect("reinstall must not fail"),
            "second install must be a no-op"
        );
        let content2 = std::fs::read_to_string(&rc).expect("read rc again");
        assert_eq!(
            content2.matches(marker.as_str()).count(),
            1,
            "marker must appear exactly once"
        );
    }

    #[test]
    fn append_creates_missing_file_and_parents() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rc = tmp.path().join("sub").join("dir").join("config.fish");
        assert!(
            append_snippet_if_missing(&rc, ShellKind::Fish).expect("append must not fail"),
            "must create parents and file"
        );
        let content = std::fs::read_to_string(&rc).expect("read rc");
        assert!(content.contains(snippet_marker(ShellKind::Fish).as_str()));
    }

    #[test]
    fn state_change_detection() {
        let keys = vec!["A".to_string(), "B".to_string()];
        // Entering: nothing exported before.
        assert!(hook_state_changed(&keys, &[]));
        // Steady state: same set, any order, duplicates tolerated.
        assert!(!hook_state_changed(
            &keys,
            &["B".to_string(), "A".to_string(), "A".to_string()]
        ));
        // Vault changed: key added or removed.
        assert!(hook_state_changed(
            &["A".to_string()],
            &["A".to_string(), "B".to_string()]
        ));
        assert!(hook_state_changed(
            &["A".to_string(), "B".to_string(), "C".to_string()],
            &["A".to_string(), "B".to_string()]
        ));
        // Poisoned tracking entries are ignored, not acted on.
        assert!(!hook_state_changed(
            &keys,
            &["A".to_string(), "B".to_string(), "EVIL;rm -rf".to_string()]
        ));
    }

    #[test]
    fn rc_paths_and_oneliners() {
        // No env mutation: only assert suffixes against the real $HOME.
        let bash = rc_file_for_shell(ShellKind::Bash).expect("bash has an rc");
        assert!(bash.ends_with(".bashrc"), "got: {}", bash.display());
        let zsh = rc_file_for_shell(ShellKind::Zsh).expect("zsh has an rc");
        assert!(zsh.ends_with(".zshrc"), "got: {}", zsh.display());
        let fish = rc_file_for_shell(ShellKind::Fish).expect("fish has an rc");
        assert!(
            fish.ends_with(".config/fish/config.fish"),
            "got: {}",
            fish.display()
        );
        assert!(rc_file_for_shell(ShellKind::Powershell).is_none());
        assert!(rc_file_for_shell(ShellKind::Nushell).is_none());
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
            ShellKind::Nushell,
        ] {
            assert!(
                oneliner_for_shell(shell).contains("shell-init"),
                "oneliner for {}",
                shell.name()
            );
        }
    }
}
