# Native Host Approval 设计

## [S1] 概述

本设计为 `kunkka-native-host` 增加 approval 消费能力。浏览器插件通过 Native Messaging 协议发送 approval 相关命令，native-host 桥接到 core 的 approval control 协议，返回结果。

这是 approval 消费链路的最后一个前端入口：CLI 已完成、TUI 已完成、native-host + 浏览器插件本次完成。

## [S2] NativeCommand 扩展

```rust
pub enum NativeCommand {
    Ping,
    Status,
    Dispatch { app_id: String, method: String, payload: serde_json::Value },
    ApprovalsList,
    ApprovalApprove { approval_id: String },
    ApprovalReject { approval_id: String },
}
```

JSON 协议示例：
- `{"id": "1", "command": "approvals_list"}`
- `{"id": "2", "command": "approval_approve", "approval_id": "appr_1"}`
- `{"id": "3", "command": "approval_reject", "approval_id": "appr_1"}`

## [S3] NativeResult 扩展

```rust
pub struct NativePendingApproval {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}

pub enum NativeResult {
    Pong,
    Status { worker_count: u64, socket_path: String, runtime_ready: bool },
    Dispatch { payload: serde_json::Value },
    DispatchError { code: String, message: String },
    PendingApprovals { approvals: Vec<NativePendingApproval> },
    ApprovalDecision,
}
```

JSON 协议示例：
- `{"id": "1", "ok": true, "result": {"type": "pending_approvals", "approvals": [...]}}`
- `{"id": "2", "ok": true, "result": {"type": "approval_decision"}}`

## [S4] Bridge 处理

`bridge.rs` 扩展：

1. `core_message_for_command` 新增 3 个分支
2. `native_result_for_core_response` 新增 3 个分支
3. `handle_request` 新增 3 个 match arm

复用现有 `send_core_control` 模式，不引入新的连接模型。

## [S5] 测试策略

### 单元测试

- `NativeCommand` / `NativeResult` JSON 序列化/反序列化
- `NativePendingApproval` JSON 序列化/反序列化

### 集成测试

- `bridge_mapping.rs`：approval 命令到 core control 消息的映射
- `bridge_session.rs`：approval 会话级操作

### 验证命令

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
