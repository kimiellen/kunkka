use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::app::{App, ApprovalsStatus, PendingApprovalItem, PingStatus, View};
use crate::client;

pub enum AppEvent {
    Ping(Result<String, String>),
    ApprovalsLoaded(Result<Vec<PendingApprovalItem>, String>),
    ApprovalDecision(Result<(), String>),
}

pub async fn run_event_loop(
    app: &mut App,
    mut terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
) -> std::io::Result<()> {
    let (tx, mut rx) = mpsc::channel::<AppEvent>(1);

    loop {
        terminal.draw(|f| crate::ui::render(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(key, app, &tx);
            }
        }

        while let Ok(ev) = rx.try_recv() {
            match ev {
                AppEvent::Ping(result) => {
                    app.ping_status = match result {
                        Ok(msg) => PingStatus::Ok(msg),
                        Err(msg) => PingStatus::Err(msg),
                    };
                }
                AppEvent::ApprovalsLoaded(result) => match result {
                    Ok(items) => {
                        app.set_approvals(items);
                    }
                    Err(msg) => {
                        app.approvals_status = ApprovalsStatus::Error(msg);
                    }
                },
                AppEvent::ApprovalDecision(result) => {
                    app.apply_approval_result(result);
                    // On success, automatically refresh the list.
                    if matches!(app.approvals_status, ApprovalsStatus::Loading) {
                        spawn_list_approvals(tx.clone());
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<AppEvent>) {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => {
            app.should_quit = true;
        }
        (KeyCode::Tab, _) => {
            app.toggle_view();
            // Auto-load approvals when switching to the approvals view.
            if app.current_view == View::Approvals
                && matches!(app.approvals_status, ApprovalsStatus::Idle)
            {
                app.approvals_status = ApprovalsStatus::Loading;
                spawn_list_approvals(tx.clone());
            }
        }
        (KeyCode::Enter, _) => {
            if app.current_view == View::Ping && !matches!(app.ping_status, PingStatus::Loading) {
                app.ping_status = PingStatus::Loading;
                let tx = tx.clone();
                tokio::spawn(async move {
                    let socket_path = client::resolve_socket_path();
                    let result = client::ping_core(&socket_path)
                        .await
                        .map(|_| "pong".to_string())
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::Ping(result)).await;
                });
            }
        }
        (KeyCode::Up, _) => {
            if app.current_view == View::Approvals {
                app.move_selection_up();
            }
        }
        (KeyCode::Down, _) => {
            if app.current_view == View::Approvals {
                app.move_selection_down();
            }
        }
        (KeyCode::Char('a'), _) => {
            if app.current_view == View::Approvals {
                if let Some(item) = app.selected_approval() {
                    let approval_id = item.approval_id.clone();
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let socket_path = client::resolve_socket_path();
                        let result = client::approve_pending_approval(&socket_path, approval_id)
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx.send(AppEvent::ApprovalDecision(result)).await;
                    });
                }
            }
        }
        (KeyCode::Char('r'), _) => {
            if app.current_view == View::Approvals {
                if let Some(item) = app.selected_approval() {
                    let approval_id = item.approval_id.clone();
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let socket_path = client::resolve_socket_path();
                        let result = client::reject_pending_approval(&socket_path, approval_id)
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx.send(AppEvent::ApprovalDecision(result)).await;
                    });
                }
            }
        }
        _ => {}
    }
}

fn spawn_list_approvals(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let socket_path = client::resolve_socket_path();
        let result = client::list_pending_approvals(&socket_path)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::ApprovalsLoaded(result)).await;
    });
}
