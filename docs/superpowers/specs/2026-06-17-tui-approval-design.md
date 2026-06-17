# TUI Approval 设计

## [S1] 概述

本设计为 `kunkka-tui` 增加 approval 消费能力。TUI 新增一个全屏 `Approvals` 视图，用户可浏览 pending approvals 列表、选择条目并执行 approve/reject 操作。

TUI 与 CLI 一样，通过 `kunkka.core-control.v1` 协议消费 approval control 消息。TUI 不需要 `kunkka-core` runtime 依赖，只使用 `kunkka-ipc` + `kunkka-protocol`。

## [S2] 视图模型

### View 枚举

```rust
pub enum View {
    Ping,
    Approvals,
}
```

### App 状态扩展

```rust
pub struct App {
    pub should_quit: bool,
    pub ping_status: PingStatus,
    pub current_view: View,
    pub approvals: Vec<PendingApprovalItem>,
    pub selected_index: usize,
    pub approvals_status: ApprovalsStatus,
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

默认进入 `View::Approvals`。

## [S3] 事件模型

### AppEvent 扩展

```rust
pub enum AppEvent {
    Ping(Result<String, String>),
    ApprovalsLoaded(Result<Vec<PendingApprovalItem>, String>),
    ApprovalDecision(Result<(), String>),
}
```

### 快捷键

- `Tab`：切换 `Ping` / `Approvals` 视图
- `a`：approve 选中项（仅 Approvals 视图）
- `r`：reject 选中项（仅 Approvals 视图）
- 上下箭头：移动选中项（仅 Approvals 视图）
- `Enter`：Ping 视图中触发 ping
- `q`：退出

## [S4] Approvals 视图 UI

### 布局

- 标题栏：`Kunkka TUI - Approvals`
- 列表区：每行显示一个 pending approval
  - `approval_id`
  - `app_id`
  - `summary`
- 选中项高亮
- 底部操作提示：`[a] Approve  [r] Reject  [Tab] Switch View  [q] Quit`

### 状态显示

- `Idle` / `Loading`：显示 `Loading approvals...`
- `Loaded` + 空列表：显示 `No pending approvals`
- `Loaded` + 非空：显示列表
- `Error`：显示错误信息（红色）

### 操作反馈

- approve/reject 后：
  - 成功：自动刷新列表
  - 失败：显示错误信息

## [S5] 客户端 Helpers

`client.rs` 新增三个异步函数：

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

复用现有 `send_core_control` 模式，不引入 `kunkka-core` runtime 依赖。

## [S6] 代码结构

```text
crates/kunkka-tui/src/
├── app.rs         # View 枚举、App 状态扩展
├── client.rs      # approval client helpers
├── event.rs       # AppEvent 扩展、handle_key_event 扩展
├── ui.rs          # Approvals 视图渲染
├── error.rs       # 可能需要扩展错误类型
└── lib.rs         # 模块导出
```

## [S7] 测试策略

### 单元测试

- `App` 状态转换：view 切换、选中索引移动、approve/reject 结果处理
- 客户端 helper 编解码

### 集成测试

- 使用 `test_paths()` + `prepare_core_runtime()` 模式
- 创建 pending approval（通过直接发 capability request）
- TUI client 列出 approvals 并验证返回
- TUI client approve 并验证返回
- TUI client reject 并验证返回

### 验证命令

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
