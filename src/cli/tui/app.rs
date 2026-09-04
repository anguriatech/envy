use crate::core::{EnvironmentSummary, ProjectSummary, SyncStatus};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Secrets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Tab,
    Enter,
    Space,
    Character(char),
    Backspace,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretEntry {
    pub key: String,
    pub value: Zeroizing<String>,
    pub updated_at: i64,
    pub revealed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    New {
        key: String,
        value: Zeroizing<String>,
        editing_value: bool,
        revealed: bool,
    },
    Edit {
        index: usize,
        value: Zeroizing<String>,
        revealed: bool,
    },
    Delete {
        index: usize,
    },
    DeleteProject {
        name: String,
        environment_count: usize,
        secret_count: usize,
        confirmation: String,
    },
    ConfirmImport {
        environment: String,
    },
    ConfirmSeal {
        project: String,
        environments: Vec<(String, usize)>,
        scroll: usize,
    },
    Rotate {
        environment: String,
        stage: RotateStage,
        current: Zeroizing<String>,
        new_pass: Zeroizing<String>,
        confirm: Zeroizing<String>,
        revealed: bool,
    },
    ProjectPicker {
        query: String,
        index: usize,
    },
    Passphrase {
        environment: String,
        value: Zeroizing<String>,
        purpose: PassphrasePurpose,
    },
    Help {
        scroll: usize,
    },
    Diff {
        text: String,
        scroll: usize,
    },
    Status {
        text: String,
        scroll: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphrasePurpose {
    Sync,
    Diff,
    Decrypt,
}

/// Input stage of the guided rotate flow (current → new → confirm → execute).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateStage {
    Current,
    New,
    Confirm,
}

/// A command the palette can execute. `label` is what the user matches against.
pub struct PaletteAction {
    pub id: &'static str,
    pub label: &'static str,
}

pub const PALETTE_ACTIONS: [PaletteAction; 16] = [
    PaletteAction {
        id: "new",
        label: "New secret",
    },
    PaletteAction {
        id: "edit",
        label: "Edit secret",
    },
    PaletteAction {
        id: "delete",
        label: "Delete secret",
    },
    PaletteAction {
        id: "seal",
        label: "Seal project to envy.enc",
    },
    PaletteAction {
        id: "rotate",
        label: "Rotate environment passphrase",
    },
    PaletteAction {
        id: "import",
        label: "Import environment from envy.enc",
    },
    PaletteAction {
        id: "diff",
        label: "Diff environment against envy.enc",
    },
    PaletteAction {
        id: "status",
        label: "Project status",
    },
    PaletteAction {
        id: "lock",
        label: "Lock vault",
    },
    PaletteAction {
        id: "unlock",
        label: "Unlock vault",
    },
    PaletteAction {
        id: "delete-project",
        label: "Delete project",
    },
    PaletteAction {
        id: "filter",
        label: "Filter secrets",
    },
    PaletteAction {
        id: "picker",
        label: "Switch project",
    },
    PaletteAction {
        id: "banner",
        label: "Toggle banner",
    },
    PaletteAction {
        id: "help",
        label: "Help",
    },
    PaletteAction {
        id: "quit",
        label: "Quit",
    },
];

/// Palette entries matching `query` (case-insensitive substring on the label).
pub fn palette_matches(query: &str) -> Vec<&'static str> {
    let query = query.to_ascii_lowercase();
    PALETTE_ACTIONS
        .iter()
        .filter(|action| query.is_empty() || action.label.to_ascii_lowercase().contains(&query))
        .map(|action| action.id)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarEntry {
    Project(usize),
    Environment(usize, usize),
}

pub struct App {
    pub projects: Vec<ProjectSummary>,
    pub environments: Vec<EnvironmentSummary>,
    pub secrets: Vec<SecretEntry>,
    pub sync_statuses: Vec<SyncStatus>,
    pub active_project: usize,
    pub active_environment: usize,
    pub expanded: bool,
    pub sidebar_cursor: usize,
    pub secret_index: usize,
    pub focus: Focus,
    pub search: String,
    pub search_active: bool,
    pub command_mode: bool,
    pub command_query: String,
    pub palette_index: usize,
    pub compact_banner: bool,
    pub vault_state: VaultState,
    pub popup: Option<Popup>,
    pub status: String,
    pub status_is_error: bool,
    pub working: bool,
    /// Name of the launch directory shown in the compact banner strip.
    pub workspace_name: String,
    /// Compact display form of the envy.enc location (never a secret).
    pub artifact_path: String,
}

impl App {
    pub fn new(projects: Vec<ProjectSummary>, expanded_banner: bool) -> Self {
        Self {
            projects,
            environments: Vec::new(),
            secrets: Vec::new(),
            sync_statuses: Vec::new(),
            active_project: 0,
            active_environment: 0,
            expanded: false,
            sidebar_cursor: 0,
            secret_index: 0,
            focus: Focus::Sidebar,
            search: String::new(),
            search_active: false,
            command_mode: false,
            command_query: String::new(),
            palette_index: 0,
            compact_banner: !expanded_banner,
            vault_state: VaultState::Unlocked,
            popup: None,
            status: String::from("Ready — press ? for help"),
            status_is_error: false,
            working: false,
            workspace_name: String::from("envy"),
            artifact_path: String::new(),
        }
    }

    /// Compact envy.enc label for the inspector: `<parent-dir>/envy.enc`.
    pub fn artifact_context(&self) -> &str {
        &self.artifact_path
    }

    /// Record an informational status message (rendered normally).
    pub fn note(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_is_error = false;
    }

    /// Record an error status message (rendered highlighted so it stands out).
    pub fn fail(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_is_error = true;
    }

    pub fn active_environment(&self) -> Option<&EnvironmentSummary> {
        self.environments.get(self.active_environment)
    }

    pub fn flatten_sidebar(&self) -> Vec<SidebarEntry> {
        let mut entries = Vec::new();
        for (pi, _) in self.projects.iter().enumerate() {
            entries.push(SidebarEntry::Project(pi));
            if self.expanded && pi == self.active_project {
                for (ei, _) in self.environments.iter().enumerate() {
                    entries.push(SidebarEntry::Environment(pi, ei));
                }
            }
        }
        entries
    }

    pub fn current_sidebar_entry(&self) -> Option<SidebarEntry> {
        self.flatten_sidebar().get(self.sidebar_cursor).copied()
    }

    pub fn move_sidebar_cursor(&mut self, delta: isize) {
        let length = self.flatten_sidebar().len();
        if length == 0 {
            return;
        }
        self.sidebar_cursor =
            (self.sidebar_cursor as isize + delta).clamp(0, length as isize - 1) as usize;
    }

    pub fn filtered_secret_indices(&self) -> Vec<usize> {
        let query = self.search.to_ascii_lowercase();
        self.secrets
            .iter()
            .enumerate()
            .filter(|(_, secret)| {
                query.is_empty() || secret.key.to_ascii_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn current_secret_index(&self) -> Option<usize> {
        self.filtered_secret_indices()
            .get(self.secret_index)
            .copied()
    }

    pub fn toggle_banner(&mut self) {
        self.compact_banner = !self.compact_banner;
    }

    pub fn set_environments(&mut self, environments: Vec<EnvironmentSummary>) {
        self.environments = environments;
        self.active_environment = 0;
        self.secret_index = 0;
        self.secrets.clear();
        self.sync_statuses.clear();
    }

    pub fn set_secrets(&mut self, secrets: Vec<SecretEntry>) {
        self.secrets = secrets;
        self.secret_index = 0;
    }

    pub fn set_sync_statuses(&mut self, statuses: Vec<SyncStatus>) {
        self.sync_statuses = statuses;
    }

    pub fn lock(&mut self) {
        self.vault_state = VaultState::Locked;
        self.secrets.clear();
        self.sync_statuses.clear();
        self.popup = None;
        self.expanded = false;
    }

    /// Index of the project whose id matches `id`, if present.
    pub fn project_index_by_id(projects: &[ProjectSummary], id: &str) -> Option<usize> {
        projects
            .iter()
            .position(|project| project.id.as_str() == id)
    }

    pub fn filtered_project_indices(&self, query: &str) -> Vec<usize> {
        let query = query.to_ascii_lowercase();
        self.projects
            .iter()
            .enumerate()
            .filter(|(_, project)| {
                query.is_empty() || project.name.to_ascii_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn handle_input(&mut self, input: Input) -> bool {
        if self.search_active {
            match input {
                Input::Quit => self.search_active = false,
                Input::Backspace => {
                    self.search.pop();
                    self.secret_index = 0;
                }
                Input::Character(character) if !character.is_control() => {
                    self.search.push(character);
                    self.secret_index = 0;
                }
                _ => {}
            }
            return false;
        }
        match input {
            Input::Quit => return true,
            Input::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Secrets,
                    _ => Focus::Sidebar,
                }
            }
            Input::Up => self.move_selection(-1),
            Input::Down => self.move_selection(1),
            Input::Space if self.focus == Focus::Secrets => {
                if let Some(index) = self.current_secret_index() {
                    self.secrets[index].revealed = !self.secrets[index].revealed;
                }
            }
            Input::Character('b') | Input::Character('B') => self.toggle_banner(),
            Input::Character('f') | Input::Character('F') => self.search_active = true,
            _ => {}
        }
        false
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            Focus::Sidebar => self.move_sidebar_cursor(delta),
            Focus::Secrets => {
                let length = self.filtered_secret_indices().len();
                if length > 0 {
                    self.secret_index =
                        (self.secret_index as isize + delta).clamp(0, length as isize - 1) as usize;
                }
                for secret in &mut self.secrets {
                    secret.revealed = false;
                }
            }
        }
    }
}

pub fn format_timestamp(epoch: i64) -> String {
    if epoch == 0 {
        return "never".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = now.saturating_sub(epoch);
    if diff < 0 {
        return "future".into();
    }
    if diff < 60 {
        "just now".into()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 86400 * 30 {
        format!("{}d ago", diff / 86400)
    } else if diff < 86400 * 365 {
        format!("{}mo ago", diff / (86400 * 30))
    } else {
        format!("{}y ago", diff / (86400 * 365))
    }
}

pub fn sync_status_icon(status: &SyncStatus) -> char {
    match status {
        SyncStatus::InSync => '✓',
        SyncStatus::Modified => '~',
        SyncStatus::NeverSealed => '·',
    }
}

/// Inner (border-excluded) height of a scrollable text popup for `lines` of content.
pub fn popup_inner_height(lines: usize) -> usize {
    lines.clamp(5, 20)
}

/// Maximum scroll offset for a scrollable text popup with `lines` of content.
pub fn popup_max_scroll(lines: usize) -> usize {
    lines.saturating_sub(popup_inner_height(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(
            vec![ProjectSummary {
                id: crate::db::ProjectId("p".into()),
                name: "demo".into(),
            }],
            false,
        )
    }

    #[test]
    fn filters_secret_keys_case_insensitively() {
        let mut app = app();
        app.secrets = vec![
            SecretEntry {
                key: "DATABASE_URL".into(),
                value: Zeroizing::new("x".into()),
                updated_at: 0,
                revealed: false,
            },
            SecretEntry {
                key: "token".into(),
                value: Zeroizing::new("y".into()),
                updated_at: 0,
                revealed: false,
            },
        ];
        app.search = "data".into();
        assert_eq!(app.filtered_secret_indices(), vec![0]);
    }

    #[test]
    fn moving_selection_remasks_values() {
        let mut app = app();
        app.focus = Focus::Secrets;
        app.secrets = vec![SecretEntry {
            key: "KEY".into(),
            value: Zeroizing::new("x".into()),
            updated_at: 0,
            revealed: true,
        }];
        app.handle_input(Input::Down);
        assert!(!app.secrets[0].revealed);
    }

    #[test]
    fn sidebar_selection_keeps_environment_selection_in_bounds() {
        let mut app = app();
        app.set_environments(vec![
            EnvironmentSummary {
                id: crate::db::EnvId("dev".into()),
                name: "development".into(),
            },
            EnvironmentSummary {
                id: crate::db::EnvId("prod".into()),
                name: "production".into(),
            },
        ]);
        app.expanded = true;
        app.handle_input(Input::Down);
        app.handle_input(Input::Down);
        assert_eq!(
            app.current_sidebar_entry(),
            Some(SidebarEntry::Environment(0, 1))
        );
    }

    #[test]
    fn locking_clears_secret_values_and_popup() {
        let mut app = app();
        app.secrets.push(SecretEntry {
            key: "TOKEN".into(),
            value: Zeroizing::new("plain".into()),
            updated_at: 0,
            revealed: true,
        });
        app.popup = Some(Popup::Delete { index: 0 });
        app.lock();
        assert_eq!(app.vault_state, VaultState::Locked);
        assert!(app.secrets.is_empty());
        assert!(app.popup.is_none());
    }

    #[test]
    fn empty_environment_clears_previous_secrets() {
        let mut app = app();
        app.secrets.push(SecretEntry {
            key: "OLD".into(),
            value: Zeroizing::new("value".into()),
            updated_at: 0,
            revealed: false,
        });
        app.set_environments(Vec::new());
        assert!(app.environments.is_empty());
        assert!(app.secrets.is_empty());
        assert!(app.active_environment().is_none());
    }

    #[test]
    fn sensitive_buffer_can_be_zeroized_in_place() {
        let mut value = Zeroizing::new(String::from("secret"));
        zeroize::Zeroize::zeroize(&mut value);
        assert!(value.is_empty());
    }

    #[test]
    fn filters_projects_case_insensitively() {
        let app = App::new(
            vec![
                ProjectSummary {
                    id: crate::db::ProjectId("one".into()),
                    name: "Production".into(),
                },
                ProjectSummary {
                    id: crate::db::ProjectId("two".into()),
                    name: "Development".into(),
                },
            ],
            false,
        );
        assert_eq!(app.filtered_project_indices("prod"), vec![0]);
    }

    #[test]
    fn flatten_sidebar_shows_all_projects_and_expanded_envs() {
        let mut app = App::new(
            vec![
                ProjectSummary {
                    id: crate::db::ProjectId("a".into()),
                    name: "alpha".into(),
                },
                ProjectSummary {
                    id: crate::db::ProjectId("b".into()),
                    name: "beta".into(),
                },
            ],
            false,
        );
        app.active_project = 1;
        app.expanded = true;
        app.environments = vec![EnvironmentSummary {
            id: crate::db::EnvId("e1".into()),
            name: "development".into(),
        }];
        let flat = app.flatten_sidebar();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0], SidebarEntry::Project(0));
        assert_eq!(flat[1], SidebarEntry::Project(1));
        assert_eq!(flat[2], SidebarEntry::Environment(1, 0));
    }

    #[test]
    fn format_timestamp_returns_never_for_zero() {
        assert_eq!(format_timestamp(0), "never");
    }

    #[test]
    fn sidebar_cursor_reaches_last_project_in_long_list() {
        let projects = (0..100)
            .map(|index| ProjectSummary {
                id: crate::db::ProjectId(index.to_string()),
                name: format!("project-{index}"),
            })
            .collect();
        let mut app = App::new(projects, false);
        app.move_sidebar_cursor(1_000);
        assert_eq!(app.sidebar_cursor, 99);
        assert_eq!(app.current_sidebar_entry(), Some(SidebarEntry::Project(99)));
    }

    #[test]
    fn note_and_fail_track_error_flag() {
        let mut app = app();
        app.fail("boom");
        assert!(app.status_is_error);
        assert_eq!(app.status, "boom");
        app.note("all good");
        assert!(!app.status_is_error);
        assert_eq!(app.status, "all good");
    }

    #[test]
    fn popup_scroll_bounds_fit_content() {
        assert_eq!(popup_inner_height(3), 5);
        assert_eq!(popup_inner_height(50), 20);
        assert_eq!(popup_max_scroll(3), 0);
        assert_eq!(popup_max_scroll(30), 10);
    }

    #[test]
    fn palette_matches_filter_by_label_substring() {
        assert!(palette_matches("").len() == PALETTE_ACTIONS.len());
        assert_eq!(palette_matches("seal"), vec!["seal"]);
        assert!(palette_matches("SECRET").contains(&"new"));
        assert!(palette_matches("nonexistent-action").is_empty());
    }

    #[test]
    fn project_index_by_id_finds_and_misses() {
        let projects = vec![
            ProjectSummary {
                id: crate::db::ProjectId("a".into()),
                name: "alpha".into(),
            },
            ProjectSummary {
                id: crate::db::ProjectId("b".into()),
                name: "beta".into(),
            },
        ];
        assert_eq!(App::project_index_by_id(&projects, "b"), Some(1));
        assert_eq!(App::project_index_by_id(&projects, "zzz"), None);
    }
}
