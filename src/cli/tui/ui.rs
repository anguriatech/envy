use super::{
    app::{App, Focus, SidebarEntry, VaultState, format_timestamp, sync_status_icon},
    banner, widgets,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState},
};

pub fn draw(frame: &mut Frame, app: &App) {
    let banner_height = if app.compact_banner { 1 } else { 5 };
    let chunks = Layout::vertical([
        Constraint::Length(banner_height),
        Constraint::Length(if app.search_active { 1 } else { 0 }),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(frame.area());
    banner::render(frame, chunks[0], app);
    if app.search_active {
        frame.render_widget(
            Paragraph::new(format!(" Find: {}", app.search))
                .block(Block::bordered().borders(Borders::BOTTOM)),
            chunks[1],
        );
    }
    let body =
        Layout::horizontal([Constraint::Percentage(32), Constraint::Min(20)]).split(chunks[2]);
    draw_sidebar(frame, body[0], app);
    draw_secrets(frame, body[1], app);
    let state = match app.vault_state {
        VaultState::Locked => "[Locked]",
        VaultState::Unlocked => "[Unlocked]",
    };
    let env = app
        .active_environment()
        .map(|e| e.name.as_str())
        .unwrap_or("no environment");
    let status = if app.working {
        " Working..."
    } else {
        " [?] Help  ↑↓ Navigate  Enter Select  Tab Panel  Q Quit"
    };
    frame.render_widget(
        Paragraph::new(format!(" {state}  {env} | {}{status}", app.status)),
        chunks[3],
    );
    widgets::popup(frame, app);
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    if app.projects.is_empty() {
        frame.render_widget(
            Paragraph::new("No projects.\nRun `envy init` first.")
                .block(Block::bordered().title(" Projects").borders(Borders::ALL)),
            area,
        );
        return;
    }
    let flat = app.flatten_sidebar();
    let mut items = Vec::new();
    for (row, entry) in flat.iter().enumerate() {
        let cursor = row == app.sidebar_cursor && app.focus == Focus::Sidebar;
        let prefix = if cursor { ">" } else { " " };
        match entry {
            SidebarEntry::Project(pi) => {
                let project = &app.projects[*pi];
                let arrow = if app.expanded && *pi == app.active_project {
                    "▾"
                } else if *pi == app.active_project {
                    "▸"
                } else {
                    " "
                };
                let active = if *pi == app.active_project {
                    " ★"
                } else {
                    ""
                };
                items.push(ListItem::new(format!(
                    "{prefix}{arrow} {}{active}",
                    project.name
                )));
            }
            SidebarEntry::Environment(_pi, ei) => {
                let env = &app.environments[*ei];
                let icon = app
                    .sync_statuses
                    .get(*ei)
                    .map(sync_status_icon)
                    .unwrap_or(' ');
                let active = if *ei == app.active_environment {
                    "▶"
                } else {
                    " "
                };
                items.push(ListItem::new(format!(
                    "{prefix}   {icon} {active} {}",
                    env.name
                )));
            }
        }
    }
    let title = if app.focus == Focus::Sidebar {
        " Projects ★"
    } else {
        " Projects"
    };
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.sidebar_cursor));
    frame.render_stateful_widget(
        List::new(items).block(Block::bordered().title(title).borders(Borders::ALL)),
        area,
        &mut state,
    );
}

fn draw_secrets(frame: &mut Frame, area: Rect, app: &App) {
    if app.vault_state == VaultState::Locked {
        frame.render_widget(
            Paragraph::new("Vault locked. Press U to unlock.")
                .block(Block::bordered().title(" Secrets").borders(Borders::ALL)),
            area,
        );
        return;
    }
    if app.active_environment().is_none() {
        frame.render_widget(
            Paragraph::new("No environment selected.\nPress Enter on a project to expand, then ↓ to select an environment.")
                .block(Block::bordered().title(" Secrets").borders(Borders::ALL)),
            area,
        );
        return;
    }
    if app.filtered_secret_indices().is_empty() {
        let message = if app.search.is_empty() {
            "No secrets. Press N to create one."
        } else {
            "No matching secrets. Press Esc to clear search."
        };
        frame.render_widget(
            Paragraph::new(message)
                .block(Block::bordered().title(" Secrets").borders(Borders::ALL)),
            area,
        );
        return;
    }
    let rows = app.filtered_secret_indices().into_iter().map(|index| {
        let secret = &app.secrets[index];
        let value = if secret.revealed {
            secret.value.as_str()
        } else {
            "********"
        };
        Row::new([
            Cell::from(secret.key.clone()),
            Cell::from(value),
            Cell::from(format_timestamp(secret.updated_at)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(45),
            Constraint::Percentage(20),
        ],
    )
    .header(
        Row::new(["KEY", "VALUE", "UPDATED"]).style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::bordered()
            .title(if app.focus == Focus::Secrets {
                " Secrets ★"
            } else {
                " Secrets"
            })
            .borders(Borders::ALL),
    );
    let mut state = TableState::default();
    state.select(Some(app.secret_index));
    frame.render_stateful_widget(table, area, &mut state);
}
