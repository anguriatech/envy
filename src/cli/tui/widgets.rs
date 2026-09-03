use super::app::{App, Popup};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

pub fn popup(frame: &mut Frame, app: &App) {
    let Some(popup) = &app.popup else {
        return;
    };
    let (title, text, height) = match popup {
        Popup::New {
            key,
            value,
            editing_value,
            revealed,
        } => (
            "New secret",
            format!(
                "Key: {key}\n{}Value: {}\n\nTab=switch field  Ctrl+R=reveal  Enter=save  Esc=cancel",
                if *editing_value { "(editing) " } else { "" },
                display(value, *revealed),
            ),
            8,
        ),
        Popup::Edit {
            value, revealed, ..
        } => (
            "Edit secret",
            format!(
                "Value: {}\n\nCtrl+R=reveal  Enter=save  Esc=cancel",
                display(value, *revealed),
            ),
            7,
        ),
        Popup::Delete { .. } => (
            "Delete secret",
            "Press Enter to confirm or Esc to cancel".into(),
            5,
        ),
        Popup::DeleteProject {
            name,
            environment_count,
            secret_count,
            confirmation,
            ..
        } => (
            "Delete project — IRREVERSIBLE",
            format!(
                "Project: {name}\n\
                 This permanently deletes {environment_count} environment(s) and {secret_count} secret(s).\n\n\
                 Type the project name to confirm:\n\
                 > {confirmation}\n\n\
                 Enter=confirm  Esc=cancel",
            ),
            9,
        ),
        Popup::ProjectPicker { query, index } => {
            let matches = app.filtered_project_indices(query);
            let items: Vec<ListItem> = matches
                .iter()
                .enumerate()
                .map(|(row, project_index)| {
                    let prefix = if row == *index { ">" } else { " " };
                    ListItem::new(format!("{prefix} {}", app.projects[*project_index].name))
                })
                .collect();
            let list = List::new(items).block(
                Block::bordered()
                    .title(" ↑↓ Select  Enter Open  Esc Close ")
                    .borders(Borders::ALL),
            );
            let area = centered(frame.area(), 64, 16);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new(format!("Search: {query}")).block(
                    Block::bordered()
                        .title("Choose project")
                        .borders(Borders::ALL),
                ),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 3,
                },
            );
            let list_area = Rect {
                x: area.x,
                y: area.y + 3,
                width: area.width,
                height: area.height.saturating_sub(3),
            };
            let mut state = ratatui::widgets::ListState::default();
            state.select(Some((*index).min(matches.len().saturating_sub(1))));
            frame.render_stateful_widget(list, list_area, &mut state);
            return;
        }
        Popup::Help => (
            "Envy TUI — Help",
            "NAVIGATION\n  ↑ / ↓       Move through project tree or secrets\n  Enter / →   Expand project or select environment\n  ←           Collapse project\n  Tab         Switch sidebar and secrets panel\n  P           Search projects\n\nSECRETS\n  F           Search secret keys\n  Space       Reveal selected value\n  N / E / D   New / edit / delete secret\n  Ctrl+R      Reveal value in popup\n\nOPERATIONS\n  S           Sync / seal to envy.enc\n  Y           Import / unseal from envy.enc\n  T           Project status and stale secrets\n  G           Diff active environment\n  L / U       Lock / unlock vault\n  X           Delete project (exact name required)\n\nQ quit    Esc close popup/search    ? help"
                .into(),
            22,
        ),
        Popup::Diff { text } => (
            "Diff",
            text.clone(),
            text.lines().count().clamp(5, 20) as u16 + 2,
        ),
        Popup::Status { text } => (
            "Status",
            text.clone(),
            text.lines().count().clamp(5, 20) as u16 + 2,
        ),
        Popup::Passphrase {
            environment, value, ..
        } => (
            "Sync passphrase",
            format!(
                "{environment}\nPassphrase: {}\n\nEnter=continue  Esc=cancel",
                display(value, false),
            ),
            7,
        ),
    };
    let area = centered(frame.area(), 72, height);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text).block(Block::bordered().title(title).borders(Borders::ALL)),
        area,
    );
}

fn display(value: &str, revealed: bool) -> String {
    if revealed {
        value.to_owned()
    } else {
        "*".repeat(value.chars().count().max(8))
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width.min(area.width)) / 2,
        y: area.y + area.height.saturating_sub(height.min(area.height)) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
