# Core Control Protocol 实施计划

> **给 agentic worker：** 必须使用 superpowers:executing-plans 逐项执行本计划。步骤使用 checkbox (`- [ ]`) 语法跟踪。

**目标：** 添加一个通过 Kunkka IPC 传输的最小 core control protocol，支持 Ping/Pong 和 Status/StatusResult。

**架构：** `kunkka-ipc` 继续保持 opaque transport。`kunkka-core` 拥有 core control typed payload 和 runtime dispatch。`CoreRuntime::run_once()` 按 payload schema 分发 worker registration 和 core control message。

**技术栈：** Rust 2021、Tokio、serde、postcard、Kunkka IPC。

---

## 文件

- 新建：`crates/kunkka-core/src/control.rs`
- 修改：`crates/kunkka-core/Cargo.toml`
- 修改：`crates/kunkka-core/src/error.rs`
- 修改：`crates/kunkka-core/src/lib.rs`
- 修改：`crates/kunkka-core/src/runtime.rs`
- 新建：`crates/kunkka-core/tests/core_control_protocol.rs`
- 新建：`crates/kunkka-core/tests/core_runtime_control.rs`
- 修改：`README.md`
- 修改：`docs/ipc.md`
- 修改：`docs/architecture.md`

## 后续迁移：Core 使用共享 Core-Control Protocol

Task 1 已将 core-control codec 所有权迁移到 `kunkka-protocol`。Task 2 仅迁移 `kunkka-core` 到共享协议，不启动 Task 3 或 native-host 工作。

### Task 2：Migrate Core to Shared Core-Control Protocol

- [ ] 先修改 `crates/kunkka-core/tests/core_runtime_control.rs` 的导入，改为使用 `kunkka_protocol::core_control`，并运行 `cargo test -p kunkka-core --test core_runtime_control` 确认因缺少 `kunkka-protocol` 依赖而 RED。
- [ ] 在 `crates/kunkka-core/Cargo.toml` 添加 `kunkka-protocol` 依赖，并从 core 移除本地 `control` 模块导出。
- [ ] 更新 `crates/kunkka-core/src/error.rs`，将 protocol codec 错误来源切换为 `kunkka_protocol::ProtocolError`。
- [ ] 更新 `crates/kunkka-core/src/runtime.rs`，使用 `kunkka_protocol::core_control` 的消息、codec 与 schema；构造 `CoreStatusResponse` 时将 `self.registry.len()` 显式转换为 `u64`。
- [ ] 删除 `crates/kunkka-core/src/control.rs` 与 `crates/kunkka-core/tests/core_control_protocol.rs`，因为 codec 所有权已迁移到 `kunkka-protocol`。
- [ ] 运行 `cargo test -p kunkka-core --test core_runtime_control`、`cargo test -p kunkka-protocol --test core_control`、`cargo fmt --all --check` 确认 GREEN。
- [ ] 按 Task 2 要求暂存并提交：`refactor: use shared core control protocol`。

### 任务 1：Core control payload codec

- [ ] **步骤 1：编写失败的 codec 测试**

创建 `crates/kunkka-core/tests/core_control_protocol.rs`，覆盖 Ping 和 StatusResult payload roundtrip。

- [ ] **步骤 2：验证 RED**

运行：`cargo test -p kunkka-core --test core_control_protocol`

预期：失败，因为 `kunkka_core::control` 尚不存在。

- [ ] **步骤 3：实现 control 类型和 codec**

创建 `crates/kunkka-core/src/control.rs`，包含：

- `CORE_CONTROL_CONTENT_TYPE`
- `CORE_CONTROL_SCHEMA`
- `CorePingRequest`
- `CorePingResponse`
- `CoreStatusRequest`
- `CoreStatusResponse`
- `CoreControlMessage`
- `encode_control_message`
- `decode_control_message`

向 `crates/kunkka-core/Cargo.toml` 添加 `serde.workspace = true` 和 `postcard.workspace = true`。

从 `crates/kunkka-core/src/lib.rs` 导出 `pub mod control;`。

- [ ] **步骤 4：验证 GREEN**

运行：`cargo test -p kunkka-core --test core_control_protocol`

预期：通过。

### 任务 2：Runtime Ping/Pong 分发

- [ ] **步骤 1：编写失败的 Ping runtime 测试**

创建 `crates/kunkka-core/tests/core_runtime_control.rs`，测试通过 `IpcConnection` 发送 `CoreControlMessage::Ping` frame，运行 `runtime.run_once()`，并收到 `CoreControlMessage::Pong`。

- [ ] **步骤 2：验证 RED**

运行：`cargo test -p kunkka-core --test core_runtime_control ping_returns_pong`

预期：失败，因为 runtime dispatch 尚未处理 core-control schema。

- [ ] **步骤 3：实现 runtime frame dispatch**

更新 `CoreRuntime`：

- 添加 `handle_frame(&mut self, frame: Frame) -> Result<Frame>`
- worker schema 分发到 `handle_worker_registration_frame`
- core control schema 分发到 control handler
- 未知 schema 返回 `CoreError::InvalidCoreFrame`

为 `CoreError` 添加 `InvalidCoreFrame(String)`。

- [ ] **步骤 4：验证 GREEN**

运行：`cargo test -p kunkka-core --test core_runtime_control ping_returns_pong`

预期：通过。

### 任务 3：Runtime Status 分发

- [ ] **步骤 1：编写失败的 Status runtime 测试**

添加一个测试：先注册一个 worker，再发送 `CoreControlMessage::Status`，针对 status request 运行 `runtime.run_once()`，并验证：

- `worker_count == 1`
- `runtime_ready == true`
- `socket_path` 匹配测试 socket path

- [ ] **步骤 2：验证 RED**

运行：`cargo test -p kunkka-core --test core_runtime_control status_returns_runtime_state`

预期：失败，直到 Status handler 返回 runtime state。

- [ ] **步骤 3：实现 Status handler**

使用 `self.registry.len()` 和 `self.server.socket_path()` 构造 `CoreStatusResponse`。

- [ ] **步骤 4：验证 GREEN**

运行：`cargo test -p kunkka-core --test core_runtime_control status_returns_runtime_state`

预期：通过。

### 任务 4：文档更新和完整验证

- [ ] **步骤 1：更新文档**

更新：

- `README.md`
- `docs/ipc.md`
- `docs/architecture.md`

- [ ] **步骤 2：格式检查**

运行：`cargo fmt --all --check`

预期：通过。

- [ ] **步骤 3：Workspace 测试**

运行：`cargo test --workspace`

预期：通过。

- [ ] **步骤 4：Clippy**

运行：`cargo clippy --workspace --all-targets -- -D warnings`

预期：通过。

- [ ] **步骤 5：Git status**

运行：`git status --short`

预期：只存在预期变更。
