mod app;
mod banner;
mod ops;
mod theme;
mod ui;
mod widgets;

use crate::{cli::CliError, db::ProjectId};
use app::{
    App, Focus, Input, PassphrasePurpose, Popup, SidebarEntry, VaultState, popup_max_scroll,
};
use ratatui::DefaultTerminal;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyModifiers,
};
use std::path::PathBuf;
use std::time::Duration;

struct Session {
    terminal: DefaultTerminal,
    vault: Option<crate::db::Vault>,
    key: Option<zeroize::Zeroizing<[u8; 32]>>,
    project_ids: Vec<ProjectId>,
    artifact_path: PathBuf,
    sync_queue: Vec<String>,
    rotation_reminder_days: u32,
    app: App,
}

impl Drop for Session {
    fn drop(&mut self) {
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
    let project_ids = projects.iter().map(|project| project.id.clone()).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (artifact_path, rotation_reminder_days) = manifest_context(&cwd)?;
    let mut session = Session {
        terminal: ratatui::init(),
        vault: Some(vault),
        key: Some(key),
        project_ids,
        artifact_path,
        sync_queue: Vec::new(),
        rotation_reminder_days,
        app: App::new(projects),
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

fn manifest_context(cwd: &std::path::Path) -> Result<(PathBuf, u32), CliError> {
    match crate::core::find_manifest(cwd) {
        Ok((manifest, manifest_dir)) => Ok((
            super::artifact_path(&manifest_dir),
            manifest.rotation_reminder_days,
        )),
        Err(crate::core::CoreError::ManifestNotFound) => Ok((cwd.join("envy.enc"), 90)),
        Err(error) => Err(CliError::Output(error.to_string())),
    }
}

impl Session {
    fn load_active_project(&mut self) -> Result<(), CliError> {
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
                ops::status_report(vault, &project_id, self.rotation_reminder_days)?
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
            if let Some(vault) = self.vault.take() {
                let _ = vault.close();
            }
            self.key = None;
            self.sync_queue.clear();
            self.app.lock();
            self.app.note("Vault locked — press U to unlock");
            return Ok(false);
        }
        if input == Input::Character('u') || input == Input::Character('U') {
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
            return Ok(false);
        }
        if input == Input::Character('n') || input == Input::Character('N') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            if self.project_ids.is_empty() || self.app.active_environment().is_none() {
                self.app.fail("Select an environment first");
                return Ok(false);
            }
            self.app.popup = Some(Popup::New {
                key: String::new(),
                value: zeroize::Zeroizing::new(String::new()),
                editing_value: false,
                revealed: false,
            });
            return Ok(false);
        }
        if input == Input::Character('e') || input == Input::Character('E') {
            if !self.require_unlocked() {
                return Ok(false);
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
            return Ok(false);
        }
        if input == Input::Character('d') || input == Input::Character('D') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            if let Some(index) = self.app.current_secret_index() {
                self.app.popup = Some(Popup::Delete { index });
            } else {
                self.app.fail("Select a secret first");
            }
            return Ok(false);
        }
        if input == Input::Character('x') || input == Input::Character('X') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            self.open_project_delete()?;
            return Ok(false);
        }
        if input == Input::Character('p') || input == Input::Character('P') {
            self.app.popup = Some(Popup::ProjectPicker {
                query: String::new(),
                index: self.app.active_project,
            });
            return Ok(false);
        }
        if input == Input::Character('?') {
            self.app.popup = Some(Popup::Help { scroll: 0 });
            return Ok(false);
        }
        if input == Input::Character('s') || input == Input::Character('S') {
            self.begin_sync()?;
            return Ok(false);
        }
        if input == Input::Character('t') || input == Input::Character('T') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            self.show_status()?;
            return Ok(false);
        }
        if input == Input::Character('g') || input == Input::Character('G') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            self.show_diff()?;
            return Ok(false);
        }
        if input == Input::Character('y') || input == Input::Character('Y') {
            if !self.require_unlocked() {
                return Ok(false);
            }
            self.confirm_import()?;
            return Ok(false);
        }
        Ok(self.app.handle_input(input))
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
        self.app.working = true;
        let result = self.with_unlocked(|vault, key| {
            ops::sync_environment(
                vault,
                key,
                &project,
                environment,
                passphrase,
                &self.artifact_path,
            )
        });
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
                ops::status_report(vault, &project, self.rotation_reminder_days)?
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
        self.app.projects = ops::load_projects(vault)?;
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

    fn begin_sync(&mut self) -> Result<(), CliError> {
        let Some(project) = self.project_ids.get(self.app.active_project).cloned() else {
            self.app.fail("Select a project first");
            return Ok(());
        };
        if !self.require_unlocked() {
            return Ok(());
        }
        let environments = {
            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| CliError::VaultOpen("vault is locked".into()))?;
            ops::load_environments(vault, &project)?
        };
        self.sync_queue = environments
            .into_iter()
            .map(|environment| environment.name)
            .collect();
        self.advance_sync()
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
                ops::environment_has_secrets(vault, &project, &environment)?
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
            ops::status_report(vault, &project, self.rotation_reminder_days)?
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
        let rows = ops::status_report(vault, &project, self.rotation_reminder_days)?;
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
                    self.rotation_reminder_days,
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
        let text = self.with_unlocked(|vault, key| {
            ops::env_diff(
                vault,
                key,
                &project,
                environment,
                &self.artifact_path,
                passphrase,
            )
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
        let count = self.with_unlocked(|vault, key| {
            ops::decrypt_env(
                vault,
                key,
                &project,
                environment,
                &self.artifact_path,
                passphrase,
            )
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
    fn artifact_path_uses_project_root_from_nested_directory() {
        let temp = tempfile::tempdir().expect("temp directory");
        let root = temp.path();
        std::fs::write(root.join("envy.toml"), "project_id = \"test\"\n").expect("manifest");
        let nested = root.join("src").join("nested");
        std::fs::create_dir_all(&nested).expect("nested directory");
        assert_eq!(
            manifest_context(&nested).expect("manifest context").0,
            root.join("envy.enc")
        );
    }
}
