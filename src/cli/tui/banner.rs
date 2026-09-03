use super::{app::App, theme};
use ratatui::{prelude::*, widgets::Paragraph};

const LOGO: [&str; 5] = [
    " ███████╗███╗   ██╗██╗   ██╗██╗   ██╗",
    " ██╔════╝████╗  ██║╚██╗ ██╔╝╚██╗ ██╔╝",
    " █████╗  ██╔██╗ ██║ ╚████╔╝  ╚████╔╝ ",
    " ██╔══╝  ██║╚██╗██║  ╚██╔╝    ╚██╔╝  ",
    " ███████╗██║ ╚████║   ██║      ██║   ",
];

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.compact_banner {
        frame.render_widget(
            Paragraph::new(" ENVY ").style(Style::default().fg(theme::color(theme::STOPS[0]))),
            area,
        );
        return;
    }
    let lines = theme::gradient(LOGO.len())
        .into_iter()
        .zip(LOGO)
        .map(|(rgb, line)| Line::from(line).style(Style::default().fg(theme::color(rgb))))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}
