# kunkka-tui Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the kunkka-tui crate as a minimal Ratatui-based TUI frontend that can connect to kunkka-core and display ping results.

**Architecture:** Single-crate TUI application using Ratatui + crossterm for terminal UI, kunkka-ipc + kunkka-protocol for core communication. Per-operation connection model (connect-send-disconnect). Async event loop with tokio for non-blocking IPC.

**Tech Stack:** Rust, Ratatui, crossterm, tokio, kunkka-ipc, kunkka-protocol

---

## File Structure

```text
crates/kunkka-tui/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Module declarations and re-exports
│   ├── main.rs          # Terminal init, app loop, terminal restore
│   ├── app.rs           # App state machine
│   ├── event.rs         # Event loop with crossterm + async IPC
│   ├── ui.rs            # Ratatui rendering
│   ├── client.rs        # IPC client (socket path, ping_core)
│   └── error.rs         # TuiError enum
└── tests/
    └── ping.rs          # Integration test with core runtime
```

---

### Task 1: Workspace Setup

**Covers:** [S2]

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/kunkka-tui/Cargo.toml`
- Create: `crates/kunkka-tui/src/lib.rs`

- [ ] **Step 1: Add kunkka-tui to workspace members**

Edit the root `Cargo.toml` to add `"crates/kunkka-tui"` to the `members` list.

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "kunkka-tui"
version = "0.1.0"
edition = "2021"

[dependencies]
kunkka-ipc = { path = "../kunkka-ipc" }
kunkka-protocol = { path = "../kunkka-protocol" }
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
kunkka-core = { path = "../kunkka-core" }
tempfile = "3"
```

- [ ] **Step 3: Create lib.rs**

```rust
pub mod app;
pub mod client;
pub mod error;
pub mod event;
pub mod ui;
```

- [ ] **Step 4: Verify crate compiles**

Run: `cargo check -p kunkka-tui`
Expected: PASS (will fail until modules exist — create empty placeholder files)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/kunkka-tui/
git commit -m "feat: add kunkka-tui workspace skeleton"
```

---

### Task 2: Error Types

**Covers:** [S5]

**Files:**
- Create: `crates/kunkka-tui/src/error.rs`

- [ ] **Step 1: Create error.rs**

```rust
use std::fmt;

#[derive(Debug)]
pub enum TuiError {
    CoreUnavailable(String),
    CoreIpc(String),
    UnexpectedCoreResponse(String),
}

impl fmt::Display for TuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiError::CoreUnavailable(msg) => write!(f, "core unavailable: {msg}"),
            TuiError::CoreIpc(msg) => write!(f, "core IPC error: {msg}"),
            TuiError::UnexpectedCoreResponse(msg) => write!(f, "unexpected response: {msg}"),
        }
    }
}

impl std::error::Error for TuiError {}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/error.rs
git commit -m "feat: add kunkka-tui error types"
```

---

### Task 3: IPC Client

**Covers:** [S5]

**Files:**
- Create: `crates/kunkka-tui/src/client.rs`

- [ ] **Step 1: Create client.rs**

```rust
use std::path::PathBuf;

use kunkka_ipc::{EndpointId, Frame, IpcConnection, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CORE_CONTROL_SCHEMA,
};

use crate::error::TuiError;

pub fn resolve_socket_path() -> PathBuf {
    if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(&xdg_runtime);
        if path.is_absolute() {
            return path.join("kunkka").join("core.sock");
        }
    }
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
}

pub async fn ping_core(socket_path: &PathBuf) -> Result<CorePingResponse, TuiError> {
    let connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|e| TuiError::CoreUnavailable(e.to_string()))?;

    let payload = encode_control_message(&CoreControlMessage::Ping(CorePingRequest {}))
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let request = Frame::Request {
        request_id: RequestId(1),
        session_id: SessionId(1),
        source: EndpointId::new("tui"),
        target: EndpointId::new("core"),
        metadata: Default::default(),
        payload,
    };

    connection
        .send_frame(&request)
        .await
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let frame = response.ok_or_else(|| {
        TuiError::CoreIpc("connection closed before response".to_string())
    })?;

    match frame {
        Frame::Response { request_id, payload, .. } => {
            if request_id != RequestId(1) {
                return Err(TuiError::UnexpectedCoreResponse(format!(
                    "expected request_id 1, got {request_id:?}"
                )));
            }
            match decode_control_message(&payload) {
                Ok(CoreControlMessage::Pong(pong)) => Ok(pong),
                Ok(other) => Err(TuiError::UnexpectedCoreResponse(format!(
                    "expected Pong, got {other:?}"
                ))),
                Err(e) => Err(TuiError::CoreIpc(e.to_string())),
            }
        }
        other => Err(TuiError::UnexpectedCoreResponse(format!(
            "expected Response frame, got {other:?}"
        ))),
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/client.rs
git commit -m "feat: add kunkka-tui IPC client"
```

---

### Task 4: App State

**Covers:** [S4]

**Files:**
- Create: `crates/kunkka-tui/src/app.rs`

- [ ] **Step 1: Create app.rs**

```rust
pub enum PingStatus {
    Idle,
    Loading,
    Ok(String),
    Err(String),
}

pub struct App {
    pub should_quit: bool,
    pub ping_status: PingStatus,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            ping_status: PingStatus::Idle,
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/app.rs
git commit -m "feat: add kunkka-tui app state"
```

---

### Task 5: UI Rendering

**Covers:** [S3]

**Files:**
- Create: `crates/kunkka-tui/src/ui.rs`

- [ ] **Step 1: Create ui.rs**

```rust
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/ui.rs
git commit -m "feat: add kunkka-tui UI rendering"
```

---

### Task 6: Event Loop

**Covers:** [S4]

**Files:**
- Create: `crates/kunkka-tui/src/event.rs`

- [ ] **Step 1: Create event.rs**

```rust
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/event.rs
git commit -m "feat: add kunkka-tui event loop"
```

---

### Task 7: Main Entry

**Covers:** [S1, S4]

**Files:**
- Create: `crates/kunkka-tui/src/main.rs`

- [ ] **Step 1: Create main.rs**

```rust
use std::io;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use kunkka_tui::app::App;
use kunkka_tui::event::run_event_loop;

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_event_loop(&mut app, terminal).await;

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-tui/src/main.rs
git commit -m "feat: add kunkka-tui main entry"
```

---

### Task 8: Integration Test

**Covers:** [S6]

**Files:**
- Create: `crates/kunkka-tui/tests/ping.rs`

- [ ] **Step 1: Create integration test**

```rust
use std::time::Duration;

use kunkka_core::paths::KunkkaPaths;
use kunkka_core::runtime::prepare_core_runtime;
use kunkka_tui::client;

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempfile::tempdir().unwrap();
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    (root, paths)
}

#[tokio::test]
async fn tui_ping_returns_pong() {
    let (_tmp, paths) = test_paths();
    prepare_core_runtime(&paths).await.unwrap();

    let core_handle = tokio::spawn({
        let paths = paths.clone();
        async move {
            kunkka_core::runtime::run_core_runtime(&paths).await;
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = client::ping_core(&paths.socket_path).await;
    assert!(result.is_ok(), "ping should succeed: {:?}", result.err());

    core_handle.abort();
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p kunkka-tui --test ping`
Expected: PASS

- [ ] **Step 3: Run full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-tui/tests/ping.rs
git commit -m "feat: add kunkka-tui ping integration test"
```
