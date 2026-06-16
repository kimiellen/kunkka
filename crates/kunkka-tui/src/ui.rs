use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, PingStatus};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Min(1),
        ])
        .split(f.area());

    let mut lines = vec![
        Line::from("Kunkka TUI").centered(),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter] Ping Core",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    let result_line = match &app.ping_status {
        PingStatus::Idle => Line::from("Result:"),
        PingStatus::Loading => Line::from(Span::styled(
            "Result: pinging...",
            Style::default().fg(Color::Yellow),
        )),
        PingStatus::Ok(msg) => Line::from(vec![
            Span::raw("Result: "),
            Span::styled(msg, Style::default().fg(Color::Green)),
        ]),
        PingStatus::Err(msg) => Line::from(vec![
            Span::raw("Result: "),
            Span::styled(msg, Style::default().fg(Color::Red)),
        ]),
    };
    lines.push(result_line);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[q] Quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Kunkka TUI"));

    f.render_widget(paragraph, chunks[0]);
}
