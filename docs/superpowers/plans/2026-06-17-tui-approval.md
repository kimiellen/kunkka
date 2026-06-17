# TUI Approval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `kunkka-tui` 增加全屏 Approvals 视图，支持浏览 pending approvals、选择条目并执行 approve/reject。

**Architecture:** TUI 新增 View 枚举实现 Ping/Approvals 视图切换。Approvals 视图通过 core-control 协议查询/操作 pending approvals，复用现有短连接模式。事件模型扩展为支持 approvals 加载和决策结果的异步回传。

**Tech Stack:** Rust, ratatui, crossterm, Tokio, kunkka-ipc, kunkka-protocol, tempfile

---

## File Structure

```text
crates/kunkka-tui/
├── src/
│   ├── app.rs               # Modify: View 枚举、App 状态扩展
│   ├── client.rs            # Modify: approval client helpers
│   ├── error.rs             # Modify: 可能需要扩展错误类型
│   ├── event.rs             # Modify: AppEvent 扩展、handle_key_event 扩展
│   ├── lib.rs               # Modify: 模块导出
│   ├── main.rs              # Keep: 入口不变
│   └── ui.rs                # Modify: Approvals 视图渲染
└── tests/
    ├── approvals.rs         # Create: approval 集成测试
    └── ping.rs              # Keep: 现有 ping 测试
```

---

### Task 1: App 状态和客户端 Helpers

**Covers:** [S2, S3, S5]

**Files:**
- Modify: `crates/kunkka-tui/src/app.rs`
- Modify: `crates/kunkka-tui/src/client.rs`
- Modify: `crates/kunkka-tui/src/error.rs`
- Modify: `crates/kunkka-tui/src/lib.rs`

- [ ] **Step 1: Write failing client helper tests**

In `crates/kunkka-tui/tests/approvals.rs`, add tests shaped like:

```rust
#[tokio::test]
async fn tui_list_pending_approvals_returns_items() {
    // setup core runtime with pending approval
    // call tui client list_pending_approvals
    // assert returned items match
}

#[tokio::test]
async fn tui_approve_pending_approval_succeeds() {
    // setup core runtime with pending approval
    // call tui client approve_pending_approval
    // assert Ok
}

#[tokio::test]
async fn tui_reject_pending_approval_succeeds() {
    // setup core runtime with pending approval
    // call tui client reject_pending_approval
    // assert Ok
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p kunkka-tui --test approvals`
Expected: FAIL because client helpers do not exist yet

- [ ] **Step 3: Add View enum and App state extensions**

In `crates/kunkka-tui/src/app.rs`, add:

```rust
pub enum View {
    Ping,
    Approvals,
}

pub enum ApprovalsStatus {
    Idle,
    Loading,
    Loaded,
    Error(String),
}

pub struct PendingApprovalItem {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}
```

And extend `App` with `current_view`, `approvals`, `selected_index`, `approvals_status`.

- [ ] **Step 4: Add client helpers**

In `crates/kunkka-tui/src/client.rs`, add `send_core_control` helper and three approval functions:

```rust
pub async fn list_pending_approvals(
    socket_path: &PathBuf,
) -> Result<Vec<PendingApprovalItem>, TuiError> { /* core control */ }

pub async fn approve_pending_approval(
    socket_path: &PathBuf,
    approval_id: String,
) -> Result<(), TuiError> { /* core control */ }

pub async fn reject_pending_approval(
    socket_path: &PathBuf,
    approval_id: String,
) -> Result<(), TuiError> { /* core control */ }
```

- [ ] **Step 5: Run tests to verify GREEN**

Run: `cargo test -p kunkka-tui --test approvals`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-tui/src/app.rs crates/kunkka-tui/src/client.rs crates/kunkka-tui/src/error.rs crates/kunkka-tui/src/lib.rs crates/kunkka-tui/tests/approvals.rs
git commit -m "feat: add tui approval client helpers and app state"
```

---

### Task 2: Approvals 视图渲染和事件处理

**Covers:** [S3, S4, S6]

**Files:**
- Modify: `crates/kunkka-tui/src/event.rs`
- Modify: `crates/kunkka-tui/src/ui.rs`

- [ ] **Step 1: Write failing UI test**

In `crates/kunkka-tui/tests/approvals.rs`, add test:

```rust
#[test]
fn app_default_view_is_approvals() {
    let app = App::new();
    assert!(matches!(app.current_view, View::Approvals));
}
```

- [ ] **Step 2: Run test to verify RED**

Run: `cargo test -p kunkka-tui --test approvals app_default_view_is_approvals`
Expected: FAIL because `current_view` field does not exist yet

- [ ] **Step 3: Implement Approvals view rendering**

In `crates/kunkka-tui/src/ui.rs`, add `render_approvals` function:

```rust
pub fn render_approvals(f: &mut Frame, app: &App) {
    // title: "Kunkka TUI - Approvals"
    // list pending approvals with selection highlight
    // bottom bar: [a] Approve  [r] Reject  [Tab] Switch View  [q] Quit
    // handle Loading/Empty/Error states
}
```

And update `render` to dispatch based on `current_view`.

- [ ] **Step 4: Implement event handling**

In `crates/kunkka-tui/src/event.rs`, extend `AppEvent` and `handle_key_event`:

```rust
pub enum AppEvent {
    Ping(Result<String, String>),
    ApprovalsLoaded(Result<Vec<PendingApprovalItem>, String>),
    ApprovalDecision(Result<(), String>),
}
```

Add key handlers:
- `Tab`：切换视图
- `a`：approve 选中项
- `r`：reject 选中项
- 上下箭头：移动选中项

- [ ] **Step 5: Run tests to verify GREEN**

Run: `cargo test -p kunkka-tui --test approvals`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-tui/src/event.rs crates/kunkka-tui/src/ui.rs crates/kunkka-tui/tests/approvals.rs
git commit -m "feat: add tui approvals view and event handling"
```

---

### Task 3: End-to-End Verification

**Covers:** [S1, S2, S3, S4, S5, S6, S7]

**Files:**
- Test: `crates/kunkka-tui/tests/approvals.rs`
- Test: `crates/kunkka-tui/tests/ping.rs`

- [ ] **Step 1: Add explicit integration coverage**

Ensure `crates/kunkka-tui/tests/approvals.rs` covers:

```rust
#[tokio::test]
async fn tui_list_pending_approvals_returns_items() { /* ... */ }

#[tokio::test]
async fn tui_approve_pending_approval_succeeds() { /* ... */ }

#[tokio::test]
async fn tui_reject_pending_approval_succeeds() { /* ... */ }

#[test]
fn app_default_view_is_approvals() { /* ... */ }

#[test]
fn app_tab_switches_view() { /* ... */ }
```

- [ ] **Step 2: Run focused verification**

Run: `cargo test -p kunkka-tui`
Expected: PASS

- [ ] **Step 3: Run workspace verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-tui crates/kunkka-protocol docs/superpowers/specs/2026-06-17-tui-approval-design.md docs/superpowers/plans/2026-06-17-tui-approval.md
git commit -m "feat: add tui approval consumption flow"
```
