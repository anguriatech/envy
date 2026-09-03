// DESIGN CONTRACT (impeccable, seed f463bcff)
// THESIS: a three-panel console where one thing is focused at a time and the
//   focused panel's keys are always visible — the inspector column turns
//   list-browsing into object inspection; refuses the "15 global hotkeys +
//   help popup" arrangement this TUI shipped with.
// OWN-WORLD: ENVY violet gradient identity intact; neutral dark ground; violet
//   marks focus/brand only, green/amber/red mark state only; dim gray for
//   metadata; hairline bordered panels with dim caps section labels.
// STORY: find a secret, read its state (sync, staleness, timestamps) without
//   leaving the eye-line, copy it without revealing it, seal with a preview,
//   rotate when the envelope disagrees — all without touching the CLI.
// FIRST VIEWPORT: 1-line compact ENVY banner; body split tree | secrets |
//   context (context hidden under 100 cols); status row + contextual key
//   legend row pinned to the bottom.
// FORM: triptych console (brief-pinned; dice seed f463bcff dealt alternatives,
//   dashboard-grid lead declined on hierarchy-depth grounds, its zoning
//   density donated into the legend and panel rules).
// FINISH: unreviewed and undocumented is unfinished; this build ends with the
//   finish review, the verdict, DESIGN.md, and every shipping raster carrying
//   its provenance.

use super::{
    app::{App, Focus, SidebarEntry, VaultState, format_timestamp, sync_status_icon},
    banner, theme, widgets,
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
        Constraint::Length(1),
    ])
    .split(frame.area());
    banner::render(frame, chunks[0], app);
    if app.search_active {
        frame.render_widget(
            Paragraph::new(format!(
                " Find: {}  (Enter closes, Esc cancels)",
                app.search
            ))
            .block(Block::bordered().borders(Borders::BOTTOM)),
            chunks[1],
        );
    }
    // Triptych body; the inspector column only fits on wide terminals.
    let body = if frame.area().width >= 100 {
        Layout::horizontal([
            Constraint::Percentage(28),
            Constraint::Min(30),
            Constraint::Percentage(26),
        ])
        .split(chunks[2])
    } else {
        Layout::horizontal([Constraint::Percentage(32), Constraint::Min(20)]).split(chunks[2])
    };
    draw_tree(frame, body[0], app);
    draw_secrets(frame, body[1], app);
    if body.len() > 2 {
        draw_context(frame, body[2], app);
    }
    draw_status(frame, chunks[3], app);
    draw_legend(frame, chunks[4], app);
    widgets::popup(frame, app);
}

fn panel_block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    let border_style = if focused {
        Style::default().fg(theme::focus())
    } else {
        Style::default().fg(theme::dim())
    };
    let title_style = if focused {
        Style::default()
            .fg(theme::focus())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::dim())
    };
    Block::bordered()
        .title(Span::styled(format!(" {title}"), title_style))
        .border_style(border_style)
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let state = match app.vault_state {
        VaultState::Locked => "[Locked]",
        VaultState::Unlocked => "[Unlocked]",
    };
    let project = app
        .projects
        .get(app.active_project)
        .map(|p| p.name.as_str())
        .unwrap_or("no project");
    let env = app
        .active_environment()
        .map(|e| e.name.as_str())
        .unwrap_or("no environment");
    let working = if app.working { "  Working..." } else { "" };
    let status_style = if app.status_is_error {
        Style::default()
            .fg(theme::alert())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let line = Line::from(vec![
        Span::raw(format!(" {state}  {project} / {env} | ")),
        Span::styled(app.status.clone(), status_style),
        Span::styled(working.to_owned(), Style::default().fg(theme::dim())),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Contextual key legend: the focused panel declares its own keys, so nothing
/// has to be memorized or looked up in help.
fn draw_legend(frame: &mut Frame, area: Rect, app: &App) {
    let keys: &[(&str, &str)] = if app.command_mode {
        &[
            ("type", "search action"),
            ("Enter", "run"),
            ("Esc", "cancel"),
        ]
    } else if app.search_active {
        &[("type", "filter"), ("Enter", "close"), ("Esc", "cancel")]
    } else {
        match app.focus {
            Focus::Sidebar => &[
                ("↑↓", "move"),
                ("Enter/→", "expand"),
                ("←", "collapse"),
                ("Tab", "secrets"),
                (":", "commands"),
                ("?", "help"),
                ("Q", "quit"),
            ],
            Focus::Secrets => &[
                ("↑↓", "move"),
                ("Space", "reveal"),
                ("Y", "copy"),
                ("N", "new"),
                ("E", "edit"),
                ("D", "delete"),
                ("F", "filter"),
                ("Tab", "tree"),
                (":", "commands"),
                ("?", "help"),
                ("Q", "quit"),
            ],
        }
    };
    let mut spans = Vec::new();
    for (index, (key, action)) in keys.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                " · ".to_owned(),
                Style::default().fg(theme::dim()),
            ));
        }
        spans.push(Span::styled(
            format!(" {key}"),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(theme::dim()),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn sync_status_color(status: &crate::core::SyncStatus) -> Color {
    match status {
        crate::core::SyncStatus::InSync => theme::ok(),
        crate::core::SyncStatus::Modified => theme::drift(),
        crate::core::SyncStatus::NeverSealed => theme::dim(),
    }
}

fn draw_tree(frame: &mut Frame, area: Rect, app: &App) {
    if app.projects.is_empty() {
        frame.render_widget(
            Paragraph::new("No projects.\nRun `envy init` first.")
                .block(panel_block("Projects", app.focus == Focus::Sidebar)),
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
                let name_style = if *pi == app.active_project {
                    Style::default()
                        .fg(theme::focus())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::raw(format!("{prefix}{arrow} ")),
                    Span::styled(format!("{}{active}", project.name), name_style),
                ])));
            }
            SidebarEntry::Environment(_pi, ei) => {
                let env = &app.environments[*ei];
                let status = app
                    .sync_statuses
                    .get(*ei)
                    .copied()
                    .unwrap_or(crate::core::SyncStatus::NeverSealed);
                let icon = sync_status_icon(&status);
                let icon_style = Style::default().fg(sync_status_color(&status));
                let active = if *ei == app.active_environment {
                    "▶"
                } else {
                    " "
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::raw(format!("{prefix}   ")),
                    Span::styled(icon.to_string(), icon_style),
                    Span::raw(format!(" {active} {}", env.name)),
                ])));
            }
        }
    }
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.sidebar_cursor));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .block(panel_block("Projects", app.focus == Focus::Sidebar)),
        area,
        &mut state,
    );
}

fn draw_secrets(frame: &mut Frame, area: Rect, app: &App) {
    if app.vault_state == VaultState::Locked {
        frame.render_widget(
            Paragraph::new("Vault locked. Press U to unlock.")
                .block(panel_block("Secrets", app.focus == Focus::Secrets)),
            area,
        );
        return;
    }
    if app.active_environment().is_none() {
        frame.render_widget(
            Paragraph::new("No environment selected.\nPress Enter on a project to expand, then ↓ to select an environment.")
                .block(panel_block("Secrets", app.focus == Focus::Secrets)),
            area,
        );
        return;
    }
    if app.filtered_secret_indices().is_empty() {
        let message = if app.search.is_empty() {
            "No secrets. Press N to create one."
        } else {
            "No matching secrets. Press F then Backspace to clear, or Esc to close search."
        };
        frame.render_widget(
            Paragraph::new(message).block(panel_block("Secrets", app.focus == Focus::Secrets)),
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
            Cell::from(format_timestamp(secret.updated_at))
                .style(Style::default().fg(theme::dim())),
        ])
    });
    let title = if app.search.is_empty() {
        "Secrets"
    } else {
        "Secrets (filtered)"
    };
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
    .block(panel_block(title, app.focus == Focus::Secrets));
    let mut state = TableState::default();
    state.select(Some(app.secret_index));
    frame.render_stateful_widget(table, area, &mut state);
}

fn section_label(lines: &mut Vec<Line<'static>>, label: &str) {
    lines.push(Line::from(Span::styled(
        label.to_owned(),
        Style::default()
            .fg(theme::dim())
            .add_modifier(Modifier::BOLD),
    )));
}

fn field_row(lines: &mut Vec<Line<'static>>, label: &str, value: Span<'static>) {
    lines.push(Line::from(vec![
        Span::styled(format!("  {label:<11}"), Style::default().fg(theme::dim())),
        value,
    ]));
}

/// The inspector column: what the current selection IS and what it lets you do.
/// This is the product-sense panel — state the CLI can only show through
/// multiple commands, visible in one glance.
fn draw_context(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if app.vault_state == VaultState::Locked {
        section_label(&mut lines, "VAULT");
        lines.push(Line::from("  Locked".to_owned()));
        lines.push(Line::from(String::new()));
        field_row(&mut lines, "unlock", Span::raw("press U".to_owned()));
        frame.render_widget(
            Paragraph::new(lines).block(panel_block("Details", false)),
            area,
        );
        return;
    }

    match app.focus {
        Focus::Sidebar => match app.current_sidebar_entry() {
            Some(SidebarEntry::Project(pi)) => {
                let project = &app.projects[pi];
                section_label(&mut lines, "PROJECT");
                field_row(
                    &mut lines,
                    "name",
                    Span::styled(project.name.clone(), Style::default().fg(theme::focus())),
                );
                let mut in_sync = 0;
                let mut modified = 0;
                let mut never = 0;
                for status in &app.sync_statuses {
                    match status {
                        crate::core::SyncStatus::InSync => in_sync += 1,
                        crate::core::SyncStatus::Modified => modified += 1,
                        crate::core::SyncStatus::NeverSealed => never += 1,
                    }
                }
                lines.push(Line::from(String::new()));
                section_label(&mut lines, "SYNC");
                let summary = |count: usize, label: &str, color: Color| {
                    Span::styled(format!(" {count} {label}"), Style::default().fg(color))
                };
                lines.push(Line::from(vec![
                    summary(in_sync, "in sync", theme::ok()),
                    summary(modified, "modified", theme::drift()),
                    summary(never, "never sealed", theme::dim()),
                ]));
                lines.push(Line::from(String::new()));
                section_label(&mut lines, "ACTIONS");
                field_row(&mut lines, "seal all", Span::raw("S".to_owned()));
                field_row(&mut lines, "delete", Span::raw("X (exact name)".to_owned()));
            }
            Some(SidebarEntry::Environment(_pi, ei)) => {
                let env = &app.environments[ei];
                section_label(&mut lines, "ENVIRONMENT");
                field_row(
                    &mut lines,
                    "name",
                    Span::styled(env.name.clone(), Style::default().fg(theme::focus())),
                );
                let status = app
                    .sync_statuses
                    .get(ei)
                    .copied()
                    .unwrap_or(crate::core::SyncStatus::NeverSealed);
                let status_label = match status {
                    crate::core::SyncStatus::InSync => "in sync",
                    crate::core::SyncStatus::Modified => "modified",
                    crate::core::SyncStatus::NeverSealed => "never sealed",
                };
                field_row(
                    &mut lines,
                    "sync",
                    Span::styled(
                        status_label.to_owned(),
                        Style::default().fg(sync_status_color(&status)),
                    ),
                );
                field_row(
                    &mut lines,
                    "secrets",
                    Span::raw(app.secrets.len().to_string()),
                );
                lines.push(Line::from(String::new()));
                section_label(&mut lines, "ACTIONS");
                field_row(&mut lines, "diff", Span::raw("G".to_owned()));
                field_row(&mut lines, "seal", Span::raw("S".to_owned()));
                field_row(&mut lines, "import", Span::raw("Y".to_owned()));
                field_row(&mut lines, "rotate", Span::raw("R".to_owned()));
            }
            None => {
                section_label(&mut lines, "PROJECTS");
                lines.push(Line::from("  Nothing selected".to_owned()));
            }
        },
        Focus::Secrets => match app.current_secret_index() {
            Some(index) => {
                let secret = &app.secrets[index];
                section_label(&mut lines, "SECRET");
                field_row(
                    &mut lines,
                    "key",
                    Span::styled(secret.key.clone(), Style::default().fg(theme::focus())),
                );
                field_row(&mut lines, "value", Span::raw("********".to_owned()));
                field_row(
                    &mut lines,
                    "updated",
                    Span::raw(format_timestamp(secret.updated_at)),
                );
                lines.push(Line::from(String::new()));
                section_label(&mut lines, "ACTIONS");
                field_row(&mut lines, "reveal", Span::raw("Space".to_owned()));
                field_row(&mut lines, "copy", Span::raw("Y".to_owned()));
                field_row(&mut lines, "edit", Span::raw("E".to_owned()));
                field_row(&mut lines, "delete", Span::raw("D".to_owned()));
            }
            None => {
                section_label(&mut lines, "SECRETS");
                lines.push(Line::from("  Nothing selected".to_owned()));
            }
        },
    }

    if !app.artifact_context().is_empty() {
        lines.push(Line::from(String::new()));
        section_label(&mut lines, "ARTIFACT");
        field_row(
            &mut lines,
            "path",
            Span::raw(app.artifact_context().to_owned()),
        );
    }

    frame.render_widget(
        Paragraph::new(lines).block(panel_block("Details", false)),
        area,
    );
}
