mod app;
mod banner;
mod clipboard;
mod ops;
mod theme;
mod ui;
mod widgets;

use crate::{cli::CliError, core::ProjectSummary, db::ProjectId};
use app::{
    App, Focus, Input, PassphrasePurpose, Popup, RotateStage, SidebarEntry, VaultState,
    palette_matches, popup_max_scroll,
};
use ratatui::DefaultTerminal;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyModifiers,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

struct Session {
    terminal: DefaultTerminal,
    vault: Option<crate::db::Vault>,
    key: Option<zeroize::Zeroizing<[u8; 32]>>,
    project_ids: Vec<ProjectId>,
    launch_dir: PathBuf,
    artifact_paths: HashMap<String, PathBuf>,
    rotation_days: HashMap<String, u32>,
    sync_queue: Vec<String>,
    app: App,
}

impl Drop for Session {
    fn drop(&mut self) {
        // The 30s clipboard window cannot survive process exit — clear now.
        clipboard::clear_now();
        close_vault(&mut self.vault);
        let _ = std::io::stdout().execute(DisableBracketedPaste);
        ratatui::restore();
    }
}

fn close_vault(vault: &mut Option<crate::db::Vault>) {
    if let Some(vault) = vault.take() {
        let _ = vault.close();
    }
}

pub(super) fn run() -> Result<(), CliError> {
    let (vault, key) = ops::open_vault()?;
    let projects = ops::load_projects(&vault)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // FR-061: the TUI operates on the *workspace* — projects whose manifest
    // sits at or below the launch directory — so every sidebar entry carries
    // its own correct artifact path and rotation threshold (FR-022).
    let workspace = workspace_context(&cwd, &projects);
    let (manifest_project, _) = manifest_context(&cwd);
    let workspace_projects: Vec<ProjectSummary> = projects
        .iter()
        .filter(|project| workspace.contains_key(project.id.as_str()))
        .cloned()
        .collect();
    let project_ids: Vec<ProjectId> = workspace_projects
        .iter()
        .map(|project| project.id.clone())
        .collect();
    let mut app = App::new(workspace_projects);
    // Launching bare `envy` inside a project directory should land on that
    // project, not on whichever row happens to be first in the vault. A
    // manifest found *above* the launch directory belongs to a project
    // outside FR-061 scope and must not shift the (empty) workspace focus.
    if let Some(project_id) = &manifest_project
        && let Some(index) = App::project_index_by_id(&app.projects, project_id)
    {
        app.active_project = index;
    }
    let artifact_paths: HashMap<String, PathBuf> = workspace
        .iter()
        .map(|(id, project)| (id.clone(), project.artifact_path.clone()))
        .collect();
    let rotation_days: HashMap<String, u32> = workspace
        .iter()
        .map(|(id, project)| (id.clone(), project.rotation_reminder_days))
        .collect();
    let mut session = Session {
        terminal: ratatui::init(),
        vault: Some(vault),
        key: Some(key),
        project_ids,
        launch_dir: cwd,
        artifact_paths,
        rotation_days,
        sync_queue: Vec::new(),
        app,
    };
    let _ = std::io::stdout().execute(EnableBracketedPaste);
    session.app.expanded = true;
    session.load_active_project()?;
    loop {
        session
            .terminal
            .draw(|frame| ui::draw(frame, &session.app))
            .map_err(|error| CliError::Output(error.to_string()))?;
        if event::poll(Duration::from_millis(100))
            .map_err(|error| CliError::Output(error.to_string()))?
        {
            match event::read().map_err(|error| CliError::Output(error.to_string()))? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    if session.handle_key(key.code, key.modifiers)? {
                        break;
                    }
                }
                Event::Paste(text) => {
                    session.handle_paste(&text);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The manifest context of the launch directory: the nearest `envy.toml`
/// found walking upward (used for launch-focus, FR-060) with its rotation
/// threshold. Returns `(None, 90)` when no manifest exists up to the
/// filesystem root — the workspace is then whatever discovery (FR-061)
/// found below the launch directory.
fn manifest_context(cwd: &std::path::Path) -> (Option<String>, u32) {
    match crate::core::find_manifest(cwd) {
        Ok((manifest, _manifest_dir)) => {
            (Some(manifest.project_id), manifest.rotation_reminder_days)
        }
        Err(_) => (None, 90),
    }
}

/// Maps downward-discovered manifests (FR-061) to their vault rows. A
/// manifest whose `project_id` has no vault row — a leftover from tests or
/// a deleted project — is out of scope for the sidebar.
fn workspace_context(
    cwd: &std::path::Path,
    projects: &[ProjectSummary],
) -> HashMap<String, crate::core::DiscoveredProject> {
    crate::core::discover_projects(cwd)
        .into_iter()
        .filter(|project| {
            projects
                .iter()
                .any(|row| row.id.as_str() == project.project_id)
        })
        .map(|project| (project.project_id.clone(), project))
        .collect()
}

impl Session {
    fn artifact_for(&self, project: &ProjectId) -> Option<PathBuf> {
        self.artifact_paths.get(project.as_str()).cloned()
    }

    /// Per-project rotation threshold (FR-022); 90 for projects without a
    /// parsed manifest — defensive, every workspace project has one.
    fn rotation_days_for(&self, project: &ProjectId) -> u32 {
        self.rotation_days
            .get(project.as_str())
            .copied()
            .unwrap_or(90)
    }

    fn load_active_project(&mut self) -> Result<(), CliError> {
        // FR-022: the inspector shows the active project's own artifact —
        // relative to the launch directory — not a single launch-wide path.
        let artifact_label = self
            .project_ids
            .get(self.app.active_project)
            .and_then(|project| self.artifact_for(project))
            .map(|artifact| {
                let manifest_dir = artifact.parent().unwrap_or(&artifact);
                match manifest_dir.strip_prefix(&self.launch_dir) {
                    Ok(relative) if relative.as_os_str().is_empty() => "envy.enc".to_string(),
                    Ok(relative) => format!("{}/envy.enc", relative.display()),
                    Err(_) => artifact.display().to_string(),
                }
            })
            .unwrap_or_default();
        self.app.artifact_path = artifact_label;
        if self.app.vault_state == VaultState::Locked {
            return Ok(());
        }
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        if let Some(project_id) = self.project_ids.get(self.app.active_project).cloned() {
            let environments = ops::load_environments(vault, &project_id)?;
            let statuses: Vec<crate::core::SyncStatus> =
                ops::status_report(vault, &project_id, self.rotation_days_for(&project_id))?
                    .into_iter()
                    .map(|row| row.sync_status)
                    .collect();
            self.app.set_environments(environments);
            self.app.set_sync_statuses(statuses);
            if let Some(environment) = self.app.active_environment().map(|env| env.name.clone()) {
                if let Some(key) = self.key.as_ref() {
                    self.app
                        .set_secrets(ops::load_secrets(vault, key, &project_id, &environment)?);
                }
            }
        }
        Ok(())
    }

    fn load_active_environment(&mut self) -> Result<(), CliError> {
        if self.app.vault_state == VaultState::Locked {
            return Ok(());
        }
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("master key is unavailable".into()))?;
        if let Some(project_id) = self.project_ids.get(self.app.active_project).cloned() {
            if let Some(environment) = self.app.active_environment().map(|env| env.name.clone()) {
                self.app
                    .set_secrets(ops::load_secrets(vault, key, &project_id, &environment)?);
            }
        }
        Ok(())
    }

    fn move_sidebar(&mut self, delta: isize) {
        self.app.move_sidebar_cursor(delta);
    }

    fn select_sidebar_entry(&mut self) -> Result<(), CliError> {
        match self.app.current_sidebar_entry() {
            Some(SidebarEntry::Project(pi)) => {
                if pi == self.app.active_project && self.app.expanded {
                    self.app.expanded = false;
                    self.app.note("Project collapsed");
                } else {
                    self.app.active_project = pi;
                    self.app.expanded = true;
                    self.load_active_project()?;
                    self.app
                        .note("Project selected — press Down for environments");
                    self.app.sidebar_cursor = self
                        .app
                        .flatten_sidebar()
                        .iter()
                        .position(|entry| *entry == SidebarEntry::Project(pi))
                        .unwrap_or(0);
                }
            }
            Some(SidebarEntry::Environment(pi, ei)) => {
                self.app.active_project = pi;
                self.app.active_environment = ei;
                self.load_active_environment()?;
                self.app.note("Environment selected — Tab opens secrets");
            }
            None => {}
        }
        Ok(())
    }

    fn collapse_sidebar(&mut self) {
        if self.app.expanded {
            self.app.expanded = false;
            self.app.sidebar_cursor = self
                .app
                .flatten_sidebar()
                .iter()
                .position(|entry| *entry == SidebarEntry::Project(self.app.active_project))
                .unwrap_or(0);
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<bool, CliError> {
        if self.app.popup.is_some() {
            return self.handle_popup(code, modifiers);
        }
        if self.app.command_mode {
            return self.handle_command(code);
        }
        if self.app.search_active {
            match code {
                KeyCode::Esc => {
                    self.app.search_active = false;
                }
                KeyCode::Enter => {
                    self.app.search_active = false;
                }
                KeyCode::Backspace => {
                    self.app.handle_input(Input::Backspace);
                }
                KeyCode::Char(character) if !character.is_control() => {
                    self.app.handle_input(Input::Character(character));
                }
                _ => {}
            }
            return Ok(false);
        }
        if code == KeyCode::Esc {
            self.app.note("Press Q to quit");
            return Ok(false);
        }
        if code == KeyCode::Char(':') {
            self.open_command_palette();
            return Ok(false);
        }
        let input = match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Input::Quit,
            KeyCode::Up => Input::Up,
            KeyCode::Down => Input::Down,
            KeyCode::Left => Input::Left,
            KeyCode::Right => Input::Right,
            KeyCode::Tab => Input::Tab,
            KeyCode::Enter => Input::Enter,
            KeyCode::Backspace => Input::Backspace,
            KeyCode::Char(' ') => Input::Space,
            KeyCode::Char(c) => Input::Character(c),
            _ => return Ok(false),
        };

        if self.app.focus == Focus::Sidebar && matches!(input, Input::Up | Input::Down) {
            self.move_sidebar(if input == Input::Up { -1 } else { 1 });
            return Ok(false);
        }
        if self.app.focus == Focus::Sidebar && input == Input::Enter {
            self.select_sidebar_entry()?;
            return Ok(false);
        }
        if self.app.focus == Focus::Sidebar && input == Input::Right {
            self.select_sidebar_entry()?;
            return Ok(false);
        }
        if self.app.focus == Focus::Sidebar && input == Input::Left {
            self.collapse_sidebar();
            return Ok(false);
        }

        if input == Input::Character('l') || input == Input::Character('L') {
            self.action_lock();
            return Ok(false);
        }
        if input == Input::Character('u') || input == Input::Character('U') {
            self.action_unlock()?;
            return Ok(false);
        }
        if input == Input::Character('n') || input == Input::Character('N') {
            self.action_new_secret();
            return Ok(false);
        }
        if input == Input::Character('e') || input == Input::Character('E') {
            self.action_edit_secret();
            return Ok(false);
        }
        if input == Input::Character('d') || input == Input::Character('D') {
            self.action_delete_secret();
            return Ok(false);
        }
        if input == Input::Character('x') || input == Input::Character('X') {
            self.action_delete_project()?;
            return Ok(false);
        }
        if input == Input::Character('p') || input == Input::Character('P') {
            self.action_switch_project();
            return Ok(false);
        }
        if input == Input::Character('?') {
            self.action_help();
            return Ok(false);
        }
        if input == Input::Character('s') || input == Input::Character('S') {
            self.action_seal()?;
            return Ok(false);
        }
        if input == Input::Character('r') || input == Input::Character('R') {
            self.action_rotate()?;
            return Ok(false);
        }
        if input == Input::Character('t') || input == Input::Character('T') {
            self.action_status()?;
            return Ok(false);
        }
        if input == Input::Character('g') || input == Input::Character('G') {
            self.action_diff()?;
            return Ok(false);
        }
        // Panel-scoped Y: copy the secret value from the secrets panel, import
        // from the project tree.
        if input == Input::Character('y') || input == Input::Character('Y') {
            if self.app.focus == Focus::Secrets {
                self.action_copy_secret();
            } else {
                self.action_import()?;
            }
            return Ok(false);
        }
        Ok(self.app.handle_input(input))
    }

    fn open_command_palette(&mut self) {
        self.app.command_mode = true;
        self.app.command_query.clear();
        self.app.palette_index = 0;
    }

    /// Handles keys while the command palette is open.
    fn handle_command(&mut self, code: KeyCode) -> Result<bool, CliError> {
        match code {
            KeyCode::Esc => {
                self.app.command_mode = false;
            }
            KeyCode::Backspace => {
                self.app.command_query.pop();
                self.app.palette_index = 0;
            }
            KeyCode::Up => self.app.palette_index = self.app.palette_index.saturating_sub(1),
            KeyCode::Down => {
                let length = palette_matches(&self.app.command_query).len();
                if length > 0 {
                    self.app.palette_index = (self.app.palette_index + 1).min(length - 1);
                }
            }
            KeyCode::Enter => {
                let id = palette_matches(&self.app.command_query)
                    .get(self.app.palette_index)
                    .copied();
                self.app.command_mode = false;
                if let Some(id) = id {
                    return self.execute_action(id);
                }
            }
            KeyCode::Char(character) if !character.is_control() => {
                self.app.command_query.push(character);
                self.app.palette_index = 0;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Runs the palette action with `id`. Returns `Ok(true)` when the app
    /// should quit.
    fn execute_action(&mut self, id: &str) -> Result<bool, CliError> {
        match id {
            "new" => self.action_new_secret(),
            "edit" => self.action_edit_secret(),
            "delete" => self.action_delete_secret(),
            "seal" => self.action_seal()?,
            "rotate" => self.action_rotate()?,
            "import" => self.action_import()?,
            "diff" => self.action_diff()?,
            "status" => self.action_status()?,
            "lock" => self.action_lock(),
            "unlock" => self.action_unlock()?,
            "delete-project" => self.action_delete_project()?,
            "filter" => self.app.search_active = true,
            "picker" => self.action_switch_project(),
            "banner" => self.app.toggle_banner(),
            "help" => self.action_help(),
            "quit" => return Ok(true),
            _ => {}
        }
        Ok(false)
    }

    fn action_lock(&mut self) {
        if let Some(vault) = self.vault.take() {
            let _ = vault.close();
        }
        self.key = None;
        self.sync_queue.clear();
        self.app.lock();
        self.app.note("Vault locked — press U to unlock");
    }

    fn action_unlock(&mut self) -> Result<(), CliError> {
        match ops::open_vault() {
            Ok((vault, key)) => {
                self.vault = Some(vault);
                self.key = Some(key);
                self.app.vault_state = VaultState::Unlocked;
                if let Err(error) = self.load_active_project() {
                    self.app.fail(error.to_string());
                    self.app.lock();
                    self.key = None;
                    if let Some(vault) = self.vault.take() {
                        let _ = vault.close();
                    }
                } else {
                    self.app.note("Vault unlocked");
                }
            }
            Err(error) => {
                self.app.vault_state = VaultState::Locked;
                self.app.fail(error.to_string());
            }
        }
        Ok(())
    }

    fn action_new_secret(&mut self) {
        if !self.require_unlocked() {
            return;
        }
        if self.project_ids.is_empty() || self.app.active_environment().is_none() {
            self.app.fail("Select an environment first");
            return;
        }
        self.app.popup = Some(Popup::New {
            key: String::new(),
            value: zeroize::Zeroizing::new(String::new()),
            editing_value: false,
            revealed: false,
        });
    }

    fn action_edit_secret(&mut self) {
        if !self.require_unlocked() {
            return;
        }
        if let Some(index) = self.app.current_secret_index() {
            self.app.popup = Some(Popup::Edit {
                index,
                value: self.app.secrets[index].value.clone(),
                revealed: false,
            });
        } else {
            self.app.fail("Select a secret first");
        }
    }

    fn action_delete_secret(&mut self) {
        if !self.require_unlocked() {
            return;
        }
        if let Some(index) = self.app.current_secret_index() {
            self.app.popup = Some(Popup::Delete { index });
        } else {
            self.app.fail("Select a secret first");
        }
    }

    fn action_delete_project(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        self.open_project_delete()
    }

    fn action_switch_project(&mut self) {
        self.app.popup = Some(Popup::ProjectPicker {
            query: String::new(),
            index: self.app.active_project,
        });
    }

    fn action_help(&mut self) {
        self.app.popup = Some(Popup::Help { scroll: 0 });
    }

    fn action_seal(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        self.propose_seal()
    }

    fn action_rotate(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        if self.app.active_environment().is_none() {
            self.app.fail("Select an environment first");
            return Ok(());
        }
        let environment = self
            .app
            .active_environment()
            .map(|env| env.name.clone())
            .unwrap_or_default();
        self.app.popup = Some(Popup::Rotate {
            environment,
            stage: RotateStage::Current,
            current: zeroize::Zeroizing::new(String::new()),
            new_pass: zeroize::Zeroizing::new(String::new()),
            confirm: zeroize::Zeroizing::new(String::new()),
            revealed: false,
        });
        Ok(())
    }

    fn action_status(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        self.show_status()
    }

    fn action_diff(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        self.show_diff()
    }

    fn action_import(&mut self) -> Result<(), CliError> {
        if !self.require_unlocked() {
            return Ok(());
        }
        self.confirm_import()
    }

    fn action_copy_secret(&mut self) {
        let Some(index) = self.app.current_secret_index() else {
            self.app.fail("Select a secret first");
            return;
        };
        match clipboard::copy_with_autoclear(self.app.secrets[index].value.as_str()) {
            Ok(()) => self.app.note(format!(
                "Copied '{}' — clipboard clears in {}s",
                self.app.secrets[index].key,
                clipboard::AUTOCLEAR_SECS
            )),
            Err(error) => self.app.fail(format!("Clipboard unavailable: {error}")),
        }
    }

    fn require_unlocked(&mut self) -> bool {
        if self.vault.is_none() {
            self.app.fail("Vault locked — press U to unlock");
            return false;
        }
        true
    }

    fn handle_paste(&mut self, text: &str) {
        let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
        if cleaned.is_empty() {
            return;
        }
        if self.app.command_mode {
            self.app.command_query.push_str(&cleaned);
            self.app.palette_index = 0;
            return;
        }
        if self.app.search_active {
            self.app.search.push_str(&cleaned);
            self.app.secret_index = 0;
            return;
        }
        match self.app.popup.take() {
            Some(Popup::New {
                mut key,
                mut value,
                editing_value,
                revealed,
            }) => {
                if editing_value {
                    value.push_str(&cleaned);
                } else {
                    key.push_str(&cleaned);
                }
                self.app.popup = Some(Popup::New {
                    key,
                    value,
                    editing_value,
                    revealed,
                });
            }
            Some(Popup::Edit {
                index,
                mut value,
                revealed,
            }) => {
                value.push_str(&cleaned);
                self.app.popup = Some(Popup::Edit {
                    index,
                    value,
                    revealed,
                });
            }
            Some(Popup::DeleteProject {
                name,
                environment_count,
                secret_count,
                mut confirmation,
            }) => {
                confirmation.push_str(&cleaned);
                self.app.popup = Some(Popup::DeleteProject {
                    name,
                    environment_count,
                    secret_count,
                    confirmation,
                });
            }
            Some(Popup::ProjectPicker { mut query, .. }) => {
                query.push_str(&cleaned);
                self.app.popup = Some(Popup::ProjectPicker { query, index: 0 });
            }
            Some(Popup::Passphrase {
                environment,
                mut value,
                purpose,
            }) => {
                value.push_str(&cleaned);
                self.app.popup = Some(Popup::Passphrase {
                    environment,
                    value,
                    purpose,
                });
            }
            Some(Popup::Rotate {
                environment,
                stage,
                mut current,
                mut new_pass,
                mut confirm,
                revealed,
            }) => {
                match stage {
                    RotateStage::Current => current.push_str(&cleaned),
                    RotateStage::New => new_pass.push_str(&cleaned),
                    RotateStage::Confirm => confirm.push_str(&cleaned),
                }
                self.app.popup = Some(Popup::Rotate {
                    environment,
                    stage,
                    current,
                    new_pass,
                    confirm,
                    revealed,
                });
            }
            other => self.app.popup = other,
        }
    }

    fn handle_popup(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<bool, CliError> {
        let popup = self
            .app
            .popup
            .take()
            .ok_or_else(|| CliError::Output("popup state lost".into()))?;
        match popup {
            Popup::New {
                mut key,
                mut value,
                mut editing_value,
                mut revealed,
            } => {
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Tab => editing_value = !editing_value,
                    KeyCode::Backspace => {
                        if editing_value {
                            value.pop();
                        } else {
                            key.pop();
                        }
                    }
                    KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
                        revealed = !revealed
                    }
                    KeyCode::Enter if !editing_value => {
                        editing_value = true;
                    }
                    KeyCode::Enter => {
                        if let Some(project) =
                            self.project_ids.get(self.app.active_project).cloned()
                        {
                            let environment = self
                                .app
                                .active_environment()
                                .map(|item| item.name.clone())
                                .unwrap_or_default();
                            let result = self.with_unlocked(|vault, key_bytes| {
                                ops::set_secret(
                                    vault,
                                    key_bytes,
                                    &project,
                                    &environment,
                                    &key,
                                    &value,
                                )
                            });
                            match result {
                                Ok(()) => self.app.note("Secret saved"),
                                Err(error) => self.app.fail(error.to_string()),
                            }
                            self.load_active_environment()?;
                        }
                        return Ok(false);
                    }
                    KeyCode::Char(character) if !character.is_control() => {
                        if editing_value {
                            value.push(character);
                        } else {
                            key.push(character);
                        }
                    }
                    _ => {}
                }
                self.app.popup = Some(Popup::New {
                    key,
                    value,
                    editing_value,
                    revealed,
                });
            }
            Popup::Edit {
                index,
                mut value,
                mut revealed,
            } => {
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
                        revealed = !revealed
                    }
                    KeyCode::Backspace => {
                        value.pop();
                    }
                    KeyCode::Enter => {
                        if let Some(project) =
                            self.project_ids.get(self.app.active_project).cloned()
                        {
                            let environment = self
                                .app
                                .active_environment()
                                .map(|item| item.name.clone())
                                .unwrap_or_default();
                            let name = self.app.secrets[index].key.clone();
                            let result = self.with_unlocked(|vault, key_bytes| {
                                ops::set_secret(
                                    vault,
                                    key_bytes,
                                    &project,
                                    &environment,
                                    &name,
                                    &value,
                                )
                            });
                            match result {
                                Ok(()) => self.app.note("Secret updated"),
                                Err(error) => self.app.fail(error.to_string()),
                            }
                            self.load_active_environment()?;
                        }
                        return Ok(false);
                    }
                    KeyCode::Char(character) if !character.is_control() => value.push(character),
                    _ => {}
                }
                self.app.popup = Some(Popup::Edit {
                    index,
                    value,
                    revealed,
                });
            }
            Popup::Delete { index } => match code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Enter => {
                    if let Some(project) = self.project_ids.get(self.app.active_project).cloned() {
                        let environment = self
                            .app
                            .active_environment()
                            .map(|item| item.name.clone())
                            .unwrap_or_default();
                        let name = self.app.secrets[index].key.clone();
                        let result = self.with_unlocked(|vault, _| {
                            ops::delete_secret(vault, &project, &environment, &name)
                        });
                        match result {
                            Ok(()) => self.app.note("Secret deleted"),
                            Err(error) => self.app.fail(error.to_string()),
                        }
                        self.load_active_environment()?;
                    }
                    return Ok(false);
                }
                _ => self.app.popup = Some(Popup::Delete { index }),
            },
            Popup::DeleteProject {
                name,
                environment_count,
                secret_count,
                mut confirmation,
            } => {
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Backspace => {
                        confirmation.pop();
                    }
                    KeyCode::Char(character) if !character.is_control() => {
                        confirmation.push(character);
                    }
                    KeyCode::Enter => {
                        if confirmation != name {
                            self.app.fail("Type the project name exactly to confirm");
                            self.app.popup = Some(Popup::DeleteProject {
                                name,
                                environment_count,
                                secret_count,
                                confirmation,
                            });
                            return Ok(false);
                        }
                        let project_id = self
                            .project_ids
                            .get(self.app.active_project)
                            .cloned()
                            .ok_or_else(|| {
                                CliError::Output("selected project no longer exists".into())
                            })?;
                        self.with_unlocked(|vault, _| ops::delete_project(vault, &project_id))?;
                        self.refresh_projects()?;
                        self.app.note(format!("Project deleted: {name}"));
                        return Ok(false);
                    }
                    _ => {}
                }
                self.app.popup = Some(Popup::DeleteProject {
                    name,
                    environment_count,
                    secret_count,
                    confirmation,
                });
            }
            Popup::ConfirmImport { environment } => match code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Enter => {
                    self.run_import(&environment)?;
                    return Ok(false);
                }
                _ => self.app.popup = Some(Popup::ConfirmImport { environment }),
            },
            Popup::ConfirmSeal {
                project,
                environments,
                scroll,
            } => {
                if let Some(next) = self.scroll_popup(
                    code,
                    {
                        let header = 3;
                        header + environments.len()
                    },
                    scroll,
                ) {
                    self.app.popup = Some(Popup::ConfirmSeal {
                        project,
                        environments,
                        scroll: next,
                    });
                    return Ok(false);
                }
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Enter => {
                        // pop() takes from the back; reverse so execution
                        // follows the preview's reading order.
                        self.sync_queue = environments.into_iter().map(|(env, _)| env).collect();
                        self.sync_queue.reverse();
                        self.advance_sync()?;
                        return Ok(false);
                    }
                    _ => {
                        self.app.popup = Some(Popup::ConfirmSeal {
                            project,
                            environments,
                            scroll,
                        })
                    }
                }
            }
            Popup::Rotate {
                environment,
                mut stage,
                mut current,
                mut new_pass,
                mut confirm,
                mut revealed,
            } => {
                let done;
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
                        revealed = !revealed;
                        done = false;
                    }
                    KeyCode::Backspace => {
                        match stage {
                            RotateStage::Current => {
                                current.pop();
                            }
                            RotateStage::New => {
                                new_pass.pop();
                            }
                            RotateStage::Confirm => {
                                confirm.pop();
                            }
                        }
                        done = false;
                    }
                    KeyCode::Enter => {
                        match stage {
                            RotateStage::Current => {
                                if current.trim().is_empty() {
                                    self.app.fail("Current passphrase must not be empty");
                                } else {
                                    stage = RotateStage::New;
                                    revealed = false;
                                }
                                done = false;
                            }
                            RotateStage::New => {
                                if new_pass.trim().is_empty() {
                                    self.app.fail("New passphrase must not be empty");
                                } else if *new_pass == *current {
                                    self.app
                                        .fail("New passphrase must differ from the current one");
                                } else {
                                    stage = RotateStage::Confirm;
                                    revealed = false;
                                }
                                done = false;
                            }
                            RotateStage::Confirm => {
                                if *confirm != *new_pass {
                                    self.app.fail("Passphrases do not match");
                                    done = false;
                                } else {
                                    done = true;
                                }
                            }
                        }
                        if !done {
                            self.app.popup = Some(Popup::Rotate {
                                environment,
                                stage,
                                current,
                                new_pass,
                                confirm,
                                revealed,
                            });
                            return Ok(false);
                        }
                        let restore = |app: &mut App| {
                            app.popup = Some(Popup::Rotate {
                                environment: environment.clone(),
                                stage,
                                current: current.clone(),
                                new_pass: new_pass.clone(),
                                confirm: confirm.clone(),
                                revealed,
                            });
                        };
                        let Some(project) = self.project_ids.get(self.app.active_project).cloned()
                        else {
                            self.app.fail("Select a project first");
                            restore(&mut self.app);
                            return Ok(false);
                        };
                        let Some(artifact) = self.artifact_for(&project) else {
                            self.app
                                .fail("This project has no envy.toml below the launch directory");
                            restore(&mut self.app);
                            return Ok(false);
                        };
                        let result = self.with_unlocked(|vault, key| {
                            ops::rotate_environment(
                                vault,
                                key,
                                &project,
                                &environment,
                                &artifact,
                                current.as_str(),
                                new_pass.as_str(),
                            )
                        });
                        match result {
                            Ok(()) => {
                                self.app
                                    .note(format!("Passphrase rotated for '{environment}'"));
                                // Rotation refreshes the seal marker; repaint
                                // the tree/inspector sync state to match.
                                if let Some(vault) = self.vault.as_ref() {
                                    let statuses: Vec<crate::core::SyncStatus> =
                                        ops::status_report(
                                            vault,
                                            &project,
                                            self.rotation_days_for(&project),
                                        )?
                                        .into_iter()
                                        .map(|row| row.sync_status)
                                        .collect();
                                    self.app.set_sync_statuses(statuses);
                                }
                            }
                            Err(error) => {
                                // Keep the dialog open with every stage buffer
                                // intact — a typo must not cost the flow.
                                self.app.fail(error.to_string());
                                restore(&mut self.app);
                            }
                        }
                        return Ok(false);
                    }
                    KeyCode::Char(character) if !character.is_control() => {
                        match stage {
                            RotateStage::Current => current.push(character),
                            RotateStage::New => new_pass.push(character),
                            RotateStage::Confirm => confirm.push(character),
                        }
                        done = false;
                    }
                    _ => done = false,
                }
                if !done {
                    self.app.popup = Some(Popup::Rotate {
                        environment,
                        stage,
                        current,
                        new_pass,
                        confirm,
                        revealed,
                    });
                }
            }
            Popup::ProjectPicker {
                mut query,
                mut index,
            } => {
                match code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Backspace => {
                        query.pop();
                        index = 0;
                    }
                    KeyCode::Up => index = index.saturating_sub(1),
                    KeyCode::Down => {
                        let length = self.app.filtered_project_indices(&query).len();
                        if length > 0 {
                            index = (index + 1).min(length - 1);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(project_index) = self
                            .app
                            .filtered_project_indices(&query)
                            .get(index)
                            .copied()
                        {
                            self.app.active_project = project_index;
                            self.app.expanded = true;
                            self.app.popup = None;
                            self.load_active_project()?;
                            self.app.sidebar_cursor = self
                                .app
                                .flatten_sidebar()
                                .iter()
                                .position(|entry| *entry == SidebarEntry::Project(project_index))
                                .unwrap_or(0);
                        } else {
                            self.app.fail("No matching projects");
                        }
                        return Ok(false);
                    }
                    KeyCode::Char(character) if !character.is_control() => {
                        query.push(character);
                        index = 0;
                    }
                    _ => {}
                }
                self.app.popup = Some(Popup::ProjectPicker { query, index });
            }
            Popup::Help { scroll } => {
                if let Some(next) =
                    self.scroll_popup(code, widgets::HELP_TEXT.lines().count(), scroll)
                {
                    self.app.popup = Some(Popup::Help { scroll: next });
                    return Ok(false);
                }
                match code {
                    KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::Char('?')
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q') => return Ok(false),
                    _ => self.app.popup = Some(Popup::Help { scroll }),
                }
            }
            Popup::Diff { text, scroll } => {
                if let Some(next) = self.scroll_popup(code, text.lines().count(), scroll) {
                    self.app.popup = Some(Popup::Diff { text, scroll: next });
                    return Ok(false);
                }
                match code {
                    KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q')
                    | KeyCode::Char('g')
                    | KeyCode::Char('G') => return Ok(false),
                    _ => self.app.popup = Some(Popup::Diff { text, scroll }),
                }
            }
            Popup::Status { text, scroll } => {
                if let Some(next) = self.scroll_popup(code, text.lines().count(), scroll) {
                    self.app.popup = Some(Popup::Status { text, scroll: next });
                    return Ok(false);
                }
                match code {
                    KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q')
                    | KeyCode::Char('t')
                    | KeyCode::Char('T') => return Ok(false),
                    _ => self.app.popup = Some(Popup::Status { text, scroll }),
                }
            }
            Popup::Passphrase {
                environment,
                value,
                purpose,
            } => {
                let mut value = value;
                match code {
                    KeyCode::Esc => {
                        if purpose == PassphrasePurpose::Sync {
                            self.sync_queue.clear();
                        }
                        return Ok(false);
                    }
                    KeyCode::Backspace => {
                        value.pop();
                    }
                    KeyCode::Enter => {
                        match purpose {
                            PassphrasePurpose::Sync => {
                                self.sync_with_passphrase(&environment, &value)?;
                            }
                            PassphrasePurpose::Diff => {
                                self.show_diff_with_passphrase(&environment, &value)?
                            }
                            PassphrasePurpose::Decrypt => {
                                self.decrypt_with_passphrase(&environment, &value)?;
                            }
                        }
                        return Ok(false);
                    }
                    KeyCode::Char(character) if !character.is_control() => value.push(character),
                    _ => {}
                }
                self.app.popup = Some(Popup::Passphrase {
                    environment,
                    value,
                    purpose,
                });
            }
        }
        Ok(false)
    }

    /// Applies Up/Down/j/k scrolling inside a text popup; returns the new scroll
    /// offset when the key was a scroll key, or `None` for any other key.
    fn scroll_popup(&self, code: KeyCode, lines: usize, scroll: usize) -> Option<usize> {
        let max = popup_max_scroll(lines);
        match code {
            KeyCode::Up | KeyCode::Char('k') => Some(scroll.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => Some((scroll + 1).min(max)),
            KeyCode::PageUp => Some(scroll.saturating_sub(10)),
            KeyCode::PageDown => Some((scroll + 10).min(max)),
            KeyCode::Home => Some(0),
            KeyCode::End => Some(max),
            _ => None,
        }
    }
    fn sync_with_passphrase(
        &mut self,
        environment: &str,
        passphrase: &str,
    ) -> Result<(), CliError> {
        let project = match self.project_ids.get(self.app.active_project).cloned() {
            Some(project) => project,
            None => {
                self.app.fail("Select an environment first");
                return Ok(());
            }
        };
        let Some(vault) = self.vault.as_ref() else {
            self.app.fail("Vault locked — press U to unlock");
            return Ok(());
        };
        let Some(key) = self.key.as_ref() else {
            self.app.fail("Master key is unavailable");
            return Ok(());
        };
        let Some(artifact) = self.artifact_for(&project) else {
            self.app
                .fail("This project has no envy.toml below the launch directory");
            return Ok(());
        };
        self.app.working = true;
        let result =
            ops::sync_environment(vault, key, &project, environment, passphrase, &artifact);
        self.app.working = false;
        let succeeded = result.is_ok();
        match result {
            Ok(()) => self.app.note("Sync complete"),
            Err(error) => self.app.fail(error.to_string()),
        }
        if succeeded {
            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
            let statuses: Vec<crate::core::SyncStatus> =
                ops::status_report(vault, &project, self.rotation_days_for(&project))?
                    .into_iter()
                    .map(|row| row.sync_status)
                    .collect();
            self.app.set_sync_statuses(statuses);
            self.advance_sync()?;
        } else {
            self.sync_queue.clear();
        }
        Ok(())
    }

    fn open_project_delete(&mut self) -> Result<(), CliError> {
        let Some(project) = self.app.projects.get(self.app.active_project) else {
            self.app.fail("No project selected");
            return Ok(());
        };
        let (environment_count, secret_count) =
            self.with_unlocked(|vault, _| ops::project_deletion_counts(vault, &project.id))?;
        self.app.popup = Some(Popup::DeleteProject {
            name: project.name.clone(),
            environment_count,
            secret_count,
            confirmation: String::new(),
        });
        Ok(())
    }

    fn refresh_projects(&mut self) -> Result<(), CliError> {
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        // FR-061: the sidebar stays scoped to the workspace — a deleted
        // project simply leaves it, newly *discovered* projects require a
        // relaunch (discovery is launch-time only).
        self.app.projects = ops::load_projects(vault)?
            .into_iter()
            .filter(|project| self.artifact_paths.contains_key(project.id.as_str()))
            .collect();
        self.project_ids = self
            .app
            .projects
            .iter()
            .map(|project| project.id.clone())
            .collect();
        if self.app.active_project >= self.app.projects.len() {
            self.app.active_project = self.app.projects.len().saturating_sub(1);
        }
        self.app.expanded = true;
        self.load_active_project()
    }

    /// Builds the seal confirmation: the environments that will be written,
    /// with their secret counts, so the user sees the blast radius before any
    /// passphrase is requested (seal preview, FR-053).
    fn propose_seal(&mut self) -> Result<(), CliError> {
        let Some(project) = self.project_ids.get(self.app.active_project).cloned() else {
            self.app.fail("Select a project first");
            return Ok(());
        };
        let environments = {
            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
            ops::load_environments(vault, &project)?
        };
        let mut listed: Vec<(String, usize)> = Vec::new();
        for environment in environments {
            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
            let count = ops::count_secrets(vault, &project, &environment.name)?;
            if count > 0 {
                listed.push((environment.name, count));
            }
        }
        if listed.is_empty() {
            self.app.fail("No environments with secrets to seal");
            return Ok(());
        }
        let project_name = self
            .app
            .projects
            .get(self.app.active_project)
            .map(|project| project.name.clone())
            .unwrap_or_default();
        self.app.popup = Some(Popup::ConfirmSeal {
            project: project_name,
            environments: listed,
            scroll: 0,
        });
        Ok(())
    }

    fn advance_sync(&mut self) -> Result<(), CliError> {
        let project = match self.project_ids.get(self.app.active_project).cloned() {
            Some(project) => project,
            None => return Ok(()),
        };
        while let Some(environment) = self.sync_queue.pop() {
            let has_secrets = {
                let vault = self
                    .vault
                    .as_ref()
                    .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
                // A transient DB error mid-queue must not kill the session:
                // report it in the status bar and stop the queue cleanly.
                match ops::environment_has_secrets(vault, &project, &environment) {
                    Ok(has) => has,
                    Err(error) => {
                        self.sync_queue.clear();
                        self.app.fail(format!("'{environment}' skipped: {error}"));
                        return Ok(());
                    }
                }
            };
            if !has_secrets {
                continue;
            }
            match ops::resolve_passphrase(&environment) {
                Some(passphrase) => {
                    self.sync_with_passphrase(&environment, &passphrase)?;
                    return Ok(());
                }
                None => {
                    self.app.popup = Some(Popup::Passphrase {
                        environment,
                        value: zeroize::Zeroizing::new(String::new()),
                        purpose: PassphrasePurpose::Sync,
                    });
                    return Ok(());
                }
            }
        }
        self.app.note("Sync complete");
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        let statuses: Vec<crate::core::SyncStatus> =
            ops::status_report(vault, &project, self.rotation_days_for(&project))?
                .into_iter()
                .map(|row| row.sync_status)
                .collect();
        self.app.set_sync_statuses(statuses);
        Ok(())
    }

    fn show_status(&mut self) -> Result<(), CliError> {
        let project = match self.project_ids.get(self.app.active_project).cloned() {
            Some(project) => project,
            None => {
                self.app.fail("Select a project first");
                return Ok(());
            }
        };
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        let rows = ops::status_report(vault, &project, self.rotation_days_for(&project))?;
        let mut text = format!(
            "Status for {}\n\n",
            self.app.projects[self.app.active_project].name
        );
        for row in &rows {
            let status_str = match row.sync_status {
                crate::core::SyncStatus::InSync => "in sync",
                crate::core::SyncStatus::Modified => "MODIFIED",
                crate::core::SyncStatus::NeverSealed => "never sealed",
            };
            text.push_str(&format!(
                "  {:<16} {:>3} secrets  {:<12}  sealed: {}\n",
                row.name,
                row.secret_count,
                status_str,
                row.sealed_at
                    .map(app::format_timestamp)
                    .unwrap_or_else(|| "never".into()),
            ));
            if !row.stale_secrets.is_empty() {
                text.push_str(&format!(
                    "    stale (>{}d): {}\n",
                    self.rotation_days_for(&project),
                    row.stale_secrets.join(", ")
                ));
            }
        }
        self.app.popup = Some(Popup::Status { text, scroll: 0 });
        Ok(())
    }

    fn show_diff(&mut self) -> Result<(), CliError> {
        let _project = match self.project_ids.get(self.app.active_project).cloned() {
            Some(project) => project,
            None => {
                self.app.fail("Select a project first");
                return Ok(());
            }
        };
        let environment = match self.app.active_environment().map(|e| e.name.clone()) {
            Some(env) => env,
            None => {
                self.app.fail("Select an environment first");
                return Ok(());
            }
        };
        let passphrase = match ops::resolve_passphrase(&environment) {
            Some(p) => p,
            None => {
                self.app.popup = Some(Popup::Passphrase {
                    environment,
                    value: zeroize::Zeroizing::new(String::new()),
                    purpose: PassphrasePurpose::Diff,
                });
                return Ok(());
            }
        };
        self.show_diff_with_passphrase(&environment, &passphrase)
    }

    fn show_diff_with_passphrase(
        &mut self,
        environment: &str,
        passphrase: &str,
    ) -> Result<(), CliError> {
        let project = self
            .project_ids
            .get(self.app.active_project)
            .cloned()
            .ok_or_else(|| CliError::Output("Select a project first".into()))?;
        let Some(artifact) = self.artifact_for(&project) else {
            self.app
                .fail("This project has no envy.toml below the launch directory");
            return Ok(());
        };
        let text = self.with_unlocked(|vault, key| {
            ops::env_diff(vault, key, &project, environment, &artifact, passphrase)
        })?;
        self.app.popup = Some(Popup::Diff { text, scroll: 0 });
        Ok(())
    }

    fn confirm_import(&mut self) -> Result<(), CliError> {
        let _project = match self.project_ids.get(self.app.active_project).cloned() {
            Some(project) => project,
            None => {
                self.app.fail("Select a project first");
                return Ok(());
            }
        };
        let environment = match self.app.active_environment().map(|e| e.name.clone()) {
            Some(env) => env,
            None => {
                self.app.fail("Select an environment first");
                return Ok(());
            }
        };
        self.app.popup = Some(Popup::ConfirmImport { environment });
        Ok(())
    }

    fn run_import(&mut self, environment: &str) -> Result<(), CliError> {
        let passphrase = match ops::resolve_passphrase(environment) {
            Some(p) => p,
            None => {
                self.app.popup = Some(Popup::Passphrase {
                    environment: environment.to_owned(),
                    value: zeroize::Zeroizing::new(String::new()),
                    purpose: PassphrasePurpose::Decrypt,
                });
                return Ok(());
            }
        };
        self.decrypt_with_passphrase(environment, &passphrase)
    }

    fn decrypt_with_passphrase(
        &mut self,
        environment: &str,
        passphrase: &str,
    ) -> Result<(), CliError> {
        let project = self
            .project_ids
            .get(self.app.active_project)
            .cloned()
            .ok_or_else(|| CliError::Output("Select a project first".into()))?;
        let Some(artifact) = self.artifact_for(&project) else {
            self.app
                .fail("This project has no envy.toml below the launch directory");
            return Ok(());
        };
        let count = self.with_unlocked(|vault, key| {
            ops::decrypt_env(vault, key, &project, environment, &artifact, passphrase)
        })?;
        self.app
            .note(format!("Imported {count} secrets from envy.enc"));
        self.load_active_environment()?;
        Ok(())
    }

    fn with_unlocked<T>(
        &self,
        operation: impl FnOnce(&crate::db::Vault, &[u8; 32]) -> Result<T, CliError>,
    ) -> Result<T, CliError> {
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| CliError::VaultOpen("master key is unavailable".into()))?;
        operation(vault, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0xCD; 32];

    #[test]
    fn close_vault_releases_database_connection() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("vault.db");
        let vault = crate::db::Vault::open(&path, &TEST_KEY).expect("vault");
        let mut slot = Some(vault);
        close_vault(&mut slot);
        assert!(slot.is_none());
        crate::db::Vault::open(&path, &TEST_KEY).expect("vault must reopen");
    }

    #[test]
    fn manifest_context_resolves_nearest_manifest_from_nested_directory() {
        let temp = tempfile::tempdir().expect("temp directory");
        let root = temp.path();
        std::fs::write(root.join("envy.toml"), "project_id = \"test\"\n").expect("manifest");
        let nested = root.join("src").join("nested");
        std::fs::create_dir_all(&nested).expect("nested directory");
        assert_eq!(manifest_context(&nested).0.as_deref(), Some("test"));
        assert_eq!(manifest_context(&nested).1, 90);
    }

    #[test]
    fn manifest_context_falls_back_without_manifest() {
        let temp = tempfile::tempdir().expect("temp directory");
        assert_eq!(manifest_context(temp.path()).0, None);
        assert_eq!(manifest_context(temp.path()).1, 90);
    }

    #[test]
    fn workspace_context_scopes_discovery_to_vault_rows() {
        use crate::core::{ProjectSummary, create_manifest};
        use crate::db::ProjectId;
        let temp = tempfile::tempdir().expect("temp directory");
        let cwd = temp.path();
        let live = cwd.join("live");
        std::fs::create_dir(&live).expect("create dir");
        create_manifest(&live, "live-id").expect("manifest");
        // Discovered, but no vault row — must be filtered out.
        let orphan = cwd.join("orphan");
        std::fs::create_dir(&orphan).expect("create dir");
        create_manifest(&orphan, "orphan-id").expect("manifest");
        let rows = vec![ProjectSummary {
            id: ProjectId("live-id".into()),
            name: "live".into(),
        }];

        let workspace = workspace_context(cwd, &rows);
        assert_eq!(workspace.len(), 1);
        assert_eq!(workspace["live-id"].manifest_dir, live);
        assert_eq!(workspace["live-id"].artifact_path, live.join("envy.enc"));
    }

    #[test]
    fn workspace_context_requires_manifest_below_launch_dir() {
        use crate::core::{ProjectSummary, create_manifest};
        use crate::db::ProjectId;
        let temp = tempfile::tempdir().expect("temp directory");
        // A manifest *above* the launch directory is out of FR-061 scope.
        create_manifest(temp.path(), "above-id").expect("manifest");
        let cwd = temp.path().join("sub");
        std::fs::create_dir(&cwd).expect("create dir");
        let rows = vec![ProjectSummary {
            id: ProjectId("above-id".into()),
            name: "above".into(),
        }];

        assert!(workspace_context(&cwd, &rows).is_empty());
    }
}
