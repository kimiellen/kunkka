use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ApprovalsStatus, PingStatus, View};

pub fn render(f: &mut Frame, app: &App) {
    match app.current_view {
        View::Ping => render_ping(f, app),
        View::Approvals => render_approvals(f, app),
    }
}

fn render_ping(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .constraints([Constraint::Min(1)])
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
        "[Tab] Switch View  [q] Quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).centered().block(
        Block::default()
            .borders(Borders::ALL)
            .title("Kunkka TUI - Ping"),
    );

    f.render_widget(paragraph, chunks[0]);
}

fn render_approvals(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .constraints([Constraint::Min(1)])
        .split(f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Kunkka TUI - Approvals");

    match &app.approvals_status {
        ApprovalsStatus::Idle | ApprovalsStatus::Loading => {
            let paragraph = Paragraph::new("Loading approvals...")
                .centered()
                .block(block);
            f.render_widget(paragraph, chunks[0]);
        }
        ApprovalsStatus::Error(msg) => {
            let lines = vec![
                Line::from(Span::styled(
                    format!("Error: {msg}"),
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                status_line(),
            ];
            let paragraph = Paragraph::new(lines).centered().block(block);
            f.render_widget(paragraph, chunks[0]);
        }
        ApprovalsStatus::Loaded => {
            if app.approvals.is_empty() {
                let lines = vec![
                    Line::from("No pending approvals"),
                    Line::from(""),
                    status_line(),
                ];
                let paragraph = Paragraph::new(lines).centered().block(block);
                f.render_widget(paragraph, chunks[0]);
            } else {
                let items: Vec<ListItem> = app
                    .approvals
                    .iter()
                    .enumerate()
                    .map(|(i, approval)| {
                        let line = Line::from(vec![
                            Span::raw(format!(
                                "[{}] ",
                                &approval.approval_id[..approval.approval_id.len().min(8)]
                            )),
                            Span::styled(&approval.app_id, Style::default().fg(Color::Cyan)),
                            Span::raw(format!(" — {}", approval.summary)),
                        ]);
                        let style = if i == app.selected_index {
                            Style::default()
                                .bg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(line).style(style)
                    })
                    .collect();

                let list = List::new(items).block(block).highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );

                let inner_chunks = Layout::default()
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(chunks[0]);

                f.render_widget(list, inner_chunks[0]);
                f.render_widget(Paragraph::new(status_line()), inner_chunks[1]);
            }
        }
    }
}

fn status_line<'a>() -> Line<'a> {
    Line::from(Span::styled(
        "[a] Approve  [r] Reject  [Tab] Switch View  [q] Quit",
        Style::default().fg(Color::DarkGray),
    ))
}
