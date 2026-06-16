# kunkka-tui 设计

## [S1] 项目定位

kunkka-tui 是 Kunkka 的全屏 TUI frontend，基于 Ratatui + crossterm。

第一刀聚焦最小可运行骨架：能连接 core、发送 ping、显示结果。

与其他 frontend（kunkka-cli、kunkka-native-host）一样，通过 kunkka-ipc + kunkka-protocol 与 core 通信。

## [S2] Crate 结构

```text
crates/kunkka-tui/
├── Cargo.toml
├── src/
│   ├── main.rs          # 入口：初始化终端、运行 app、恢复终端
│   ├── app.rs           # App 状态机：持有 UI 状态和操作结果
│   ├── event.rs         # 事件循环：crossterm 事件 + 自定义事件
│   ├── ui.rs            # Ratatui 渲染：根据 App 状态绘制 UI
│   ├── client.rs        # IPC 客户端：复用 CLI 的连接模式
│   ├── error.rs         # 错误类型
│   └── lib.rs           # 模块导出
└── tests/
    └── ping.rs          # 集成测试：启动 core，TUI 发送 ping
```

依赖：

- `ratatui` + `crossterm`（TUI 框架）
- `kunkka-ipc` + `kunkka-protocol`（IPC 通信）
- `tokio`（异步 runtime）
- `kunkka-core`（dev-dependency，用于集成测试）

## [S3] UI 布局

```text
┌─────────────────────────────────────┐
│         Kunkka TUI                  │
│                                     │
│  [Enter] Ping Core                  │
│                                     │
│  Result: <空 / pong / 错误信息>      │
│                                     │
│  [q] Quit                           │
└─────────────────────────────────────┘
```

- 居中显示操作提示
- Enter 触发 ping
- 结果以内联文本显示在操作区域下方
- q 退出
- 错误直接显示在 Result 区域（红色）

## [S4] 事件循环与状态机

App 持有状态：

- `should_quit: bool`
- `ping_result: Option<Result<String, String>>`
- `loading: bool`

事件循环（tokio）：

1. `event::poll(Duration)` + `event::read()` 非阻塞模式产生键盘事件
2. Enter → 设 `loading = true`，spawn 异步 ping 任务
3. ping 完成 → 更新 `ping_result`，设 `loading = false`
4. 每个 tick 用 `terminal.draw(|f| ui::render(f, &app))` 刷新
5. q → 设 `should_quit = true`，退出循环

异步 ping 使用 `tokio::spawn`，通过 `tokio::sync::mpsc` 将结果送回事件循环。

## [S5] IPC 客户端

复用 CLI 的连接模式：

- `resolve_socket_path()` — XDG socket 路径解析（与 CLI/native-host 相同逻辑）
- `ping_core()` — 连接 socket → 发送 `CoreControlMessage::Ping` → 接收 `Pong` → 返回结果
- 每次操作新建连接，操作完即断开
- EndpointId: `"tui"`
- 错误类型：`CoreUnavailable`、`CoreIpc`、`UnexpectedCoreResponse`

## [S6] 测试策略

- 单元测试：`app.rs` 状态转换、`client.rs` socket 路径解析
- 集成测试（`tests/ping.rs`）：
  - 使用 `test_paths()` 模式构造临时 XDG 目录
  - 启动 `prepare_core_runtime()` 和 core runtime
  - TUI client 连接并发送 ping
  - 验证收到 pong 响应
- 验证命令：`cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
