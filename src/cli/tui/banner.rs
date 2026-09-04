use super::{app::App, theme};
use ratatui::{prelude::*, widgets::Paragraph};

/// Terminals shorter than this many rows open with the compact strip
/// instead of the full five-row gradient logo (FR-062).
pub const BANNER_MIN_HEIGHT: u16 = 32;

const LOGO: [&str; 5] = [
    " ███████╗███╗   ██╗██╗   ██╗██╗   ██╗",
    " ██╔════╝████╗  ██║╚██╗ ██╔╝╚██╗ ██╔╝",
    " █████╗  ██╔██╗ ██║ ╚████╔╝  ╚████╔╝ ",
    " ██╔══╝  ██║╚██╗██║  ╚██╔╝    ╚██╔╝  ",
    " ███████╗██║ ╚████║   ██║      ██║   ",
];

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.compact_banner {
        let spans = compact_spans(area.width, &app.workspace_name);
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }
    let lines = theme::gradient(LOGO.len())
        .into_iter()
        .zip(LOGO)
        .map(|(rgb, line)| Line::from(line).style(Style::default().fg(theme::color(rgb))))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

/// The compact identity strip: violet wordmark, then version and workspace
/// in dim gray. Narrow terminals degrade to the bare wordmark so the line
/// never clips mid-token (FR-062).
fn compact_spans(width: u16, workspace: &str) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        " ENVY ",
        Style::default()
            .fg(theme::color(theme::STOPS[0]))
            .add_modifier(Modifier::BOLD),
    )];
    if width < 40 {
        return spans;
    }
    spans.push(Span::styled(
        format!(" · v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(theme::dim()),
    ));
    spans.push(Span::styled(
        format!(" · {workspace}"),
        Style::default().fg(theme::dim()),
    ));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_text(width: u16, workspace: &str) -> Vec<String> {
        compact_spans(width, workspace)
            .iter()
            .map(|span| span.content.to_string())
            .collect()
    }

    #[test]
    fn compact_strip_is_enriched_on_wide_terminals() {
        let spans = strip_text(80, "envy-project");
        assert_eq!(spans[0], " ENVY ");
        assert!(spans[1].starts_with(" · v"), "version span: {}", spans[1]);
        assert_eq!(spans[2], " · envy-project");
    }

    #[test]
    fn compact_strip_degrades_to_wordmark_on_narrow_terminals() {
        assert_eq!(strip_text(39, "workspace"), vec![" ENVY ".to_string()]);
        assert_eq!(strip_text(120, "workspace")[0], " ENVY ");
    }

    #[test]
    fn compact_strip_never_leaks_without_workspace() {
        // The strip is identity-only metadata — no artifact or secret text.
        let spans = strip_text(80, "any-dir");
        let joined = spans.join("");
        assert!(!joined.contains("envy.enc"));
        assert!(!joined.contains('/'), "workspace label must be a bare name");
    }
}
