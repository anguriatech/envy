use super::app::{App, Popup, RotateStage, palette_matches, popup_inner_height, popup_max_scroll};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

pub const HELP_TEXT: &str = "NAVIGATION\n  ↑ / ↓, j/k  Move through the focused panel\n  Tab         Switch between Projects and Secrets\n  Enter / →   Expand project or select environment\n  ←           Collapse project\n  :           Command palette (every action, searchable)\n  Esc         Close popup or search (press Q to quit)\n\nPROJECTS PANEL\n  Enter / →   Expand and select\n  S           Seal project (preview first)\n  T           Project status\n  Y           Import active environment from envy.enc\n  R           Rotate environment passphrase\n  X           Delete project (exact name required)\n\nSECRETS PANEL\n  F           Filter keys (Enter keeps filter)\n  Space       Reveal selected value\n  Y           Copy value to clipboard (clears in 30s)\n  N / E / D   New / edit / delete secret\n  Ctrl+R      Reveal value while typing\n\nVAULT\n  S           Seal project environments to envy.enc\n  G           Diff active environment against envy.enc\n  L / U       Lock / unlock vault\n  B           Toggle banner\n\nQ quit    ? help    ↑↓ scroll this window";

pub fn popup(frame: &mut Frame, app: &App) {
    if app.command_mode {
        draw_palette(frame, app);
        return;
    }
    let Some(popup) = &app.popup else {
        return;
    };
    if let Popup::ProjectPicker { query, index } = popup {
        let matches = app.filtered_project_indices(query);
        let items: Vec<ListItem> = matches
            .iter()
            .enumerate()
            .map(|(row, project_index)| {
                let prefix = if row == *index { ">" } else { " " };
                ListItem::new(format!("{prefix} {}", app.projects[*project_index].name))
            })
            .collect();
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .block(
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

    let (title, text, lines, scroll): (String, String, usize, usize) = match popup {
        Popup::New {
            key,
            value,
            editing_value,
            revealed,
        } => (
            "New secret".to_owned(),
            format!(
                "Key: {key}\n{}Value: {}\n\nEnter=next field/save  Tab=switch field  Ctrl+R=reveal  Esc=cancel",
                if *editing_value { "(editing) " } else { "" },
                display(value, *revealed),
            ),
            0,
            0,
        ),
        Popup::Edit {
            index,
            value,
            revealed,
        } => {
            let name = app
                .secrets
                .get(*index)
                .map(|secret| secret.key.as_str())
                .unwrap_or("?");
            (
                format!("Edit secret — {name}"),
                format!(
                    "Value: {}\n\nCtrl+R=reveal  Enter=save  Esc=cancel",
                    display(value, *revealed),
                ),
                0,
                0,
            )
        }
        Popup::Delete { index } => {
            let name = app
                .secrets
                .get(*index)
                .map(|secret| secret.key.as_str())
                .unwrap_or("?");
            let environment = app
                .active_environment()
                .map(|env| env.name.as_str())
                .unwrap_or("?");
            (
                "Delete secret".to_owned(),
                format!("Delete '{name}' from {environment}?\n\nEnter=confirm  Esc=cancel"),
                0,
                0,
            )
        }
        Popup::DeleteProject {
            name,
            environment_count,
            secret_count,
            confirmation,
            ..
        } => (
            "Delete project — IRREVERSIBLE".to_owned(),
            format!(
                "Project: {name}\n\
                 This permanently deletes {environment_count} environment(s) and {secret_count} secret(s).\n\n\
                 Type the project name to confirm:\n\
                 > {confirmation}\n\n\
                 Enter=confirm  Esc=cancel",
            ),
            0,
            0,
        ),
        Popup::ConfirmImport { environment } => (
            "Import from envy.enc".to_owned(),
            format!(
                "Import environment '{environment}' from envy.enc into the vault?\n\
                 Existing secrets with the same keys will be overwritten.\n\n\
                 Enter=import  Esc=cancel"
            ),
            0,
            0,
        ),
        Popup::ConfirmSeal {
            project,
            environments,
            scroll,
        } => {
            let mut text = format!("Seal project '{project}' into envy.enc?\n\n");
            for (env, count) in environments {
                text.push_str(&format!("  {env}  ({count} secrets)\n"));
            }
            text.push_str(
                "\nEnvironments already sealed keep their passphrase.\n\nEnter=seal  Esc=cancel",
            );
            let lines = text.lines().count();
            (
                "Seal to envy.enc (↑↓ scroll)".to_owned(),
                text,
                lines,
                *scroll,
            )
        }
        Popup::Rotate {
            environment,
            stage,
            current,
            new_pass,
            confirm,
            revealed,
        } => {
            let (label, value) = match stage {
                RotateStage::Current => ("Current passphrase", current.as_str()),
                RotateStage::New => ("New passphrase", new_pass.as_str()),
                RotateStage::Confirm => ("Confirm new passphrase", confirm.as_str()),
            };
            let step = match stage {
                RotateStage::Current => "step 1/3",
                RotateStage::New => "step 2/3",
                RotateStage::Confirm => "step 3/3",
            };
            (
                format!("Rotate passphrase — {environment}"),
                format!(
                    "{step}  {label}: {}\n\nCtrl+R=reveal  Enter=next  Esc=cancel",
                    display(value, *revealed),
                ),
                0,
                0,
            )
        }
        Popup::Help { scroll } => (
            "Help (↑↓ scroll)".to_owned(),
            HELP_TEXT.to_owned(),
            HELP_TEXT.lines().count(),
            *scroll,
        ),
        Popup::Diff { text, scroll } => (
            "Diff (↑↓ scroll)".to_owned(),
            text.clone(),
            text.lines().count(),
            *scroll,
        ),
        Popup::Status { text, scroll } => (
            "Status (↑↓ scroll)".to_owned(),
            text.clone(),
            text.lines().count(),
            *scroll,
        ),
        Popup::Passphrase {
            environment,
            purpose,
            ..
        } => {
            let title = match purpose {
                super::app::PassphrasePurpose::Sync => "Sync passphrase",
                super::app::PassphrasePurpose::Diff => "Diff passphrase",
                super::app::PassphrasePurpose::Decrypt => "Import passphrase",
            };
            (
                title.to_owned(),
                format!("{environment}\nPassphrase: ********\n\nEnter=continue  Esc=cancel"),
                0,
                0,
            )
        }
        Popup::ProjectPicker { .. } => unreachable!("handled above"),
    };

    let height = if lines > 0 {
        popup_inner_height(lines) as u16 + 2
    } else {
        let counted = text.lines().count();
        counted.clamp(5, 9) as u16 + 2
    };
    let area = centered(frame.area(), 72, height);
    frame.render_widget(Clear, area);
    let mut paragraph =
        Paragraph::new(text).block(Block::bordered().title(title).borders(Borders::ALL));
    if lines > 0 && popup_max_scroll(lines) > 0 {
        paragraph = paragraph.scroll((scroll.min(popup_max_scroll(lines)) as u16, 0));
    }
    frame.render_widget(paragraph, area);
}

fn display(value: &str, revealed: bool) -> String {
    if revealed {
        value.to_owned()
    } else {
        "*".repeat(value.chars().count().max(8))
    }
}

/// The command palette: fuzzy-ish substring search over every action the TUI
/// can perform, so discoverability never depends on memorized hotkeys.
fn draw_palette(frame: &mut Frame, app: &App) {
    let matches = palette_matches(&app.command_query);
    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(row, id)| {
            let label = super::app::PALETTE_ACTIONS
                .iter()
                .find(|action| action.id == *id)
                .map(|action| action.label)
                .unwrap_or("");
            let prefix = if row == app.palette_index { ">" } else { " " };
            ListItem::new(format!("{prefix} {label}"))
        })
        .collect();
    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .block(
            Block::bordered()
                .title(" ↑↓ Select  Enter Run  Esc Close ")
                .border_style(Style::default().fg(super::theme::focus()))
                .borders(Borders::ALL),
        );
    let area = centered(frame.area(), 64, 16);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(format!("Run command: {}", app.command_query)).block(
            Block::bordered()
                .title("Commands")
                .border_style(Style::default().fg(super::theme::focus()))
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
    state.select(Some(app.palette_index.min(matches.len().saturating_sub(1))));
    frame.render_stateful_widget(list, list_area, &mut state);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width.min(area.width)) / 2,
        y: area.y + area.height.saturating_sub(height.min(area.height)) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
