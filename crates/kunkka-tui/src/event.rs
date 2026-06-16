use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::app::{App, PingStatus};
use crate::client;

pub enum AppEvent {
    Ping(Result<String, String>),
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
        (KeyCode::Enter, _) => {
            if !matches!(app.ping_status, PingStatus::Loading) {
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
        _ => {}
    }
}
