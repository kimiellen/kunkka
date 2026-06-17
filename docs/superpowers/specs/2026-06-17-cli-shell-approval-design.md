# CLI Shell Approval 设计

## [S1] 概述

本设计把 `kunkka-cli` 作为 shell approval 的第一刀前端消费入口。目标是在一次 CLI 命令中完成以下流程：

1. 发起 shell capability 请求
2. 命中 `ask` 时提示用户确认
3. 调用 core approval control 做 approve/reject
4. 批准后自动重试原 shell request
5. 输出最终 shell 执行结果

这次只覆盖 CLI，不同时实现 TUI 或浏览器插件前端 UI。它们后续复用同一套 core approval 协议接入。

## [S2] 命令模型

CLI 新增 shell capability 入口，例如：

```text
kunkka shell --app notes --command "printf foo | wc -c"
```

参数：

- `--app`：目标 app_id
- `--command`：发送给 shell capability 的原始命令字符串

第一刀不增加额外 shell 参数，不支持单独指定 `approval_id`，也不暴露批量审批命令。

## [S3] 用户流程

### 正常完成路径

1. CLI 组装 `shell.run` capability request
2. core 返回 `ShellRunOutcome::Completed`
3. CLI 输出 `stdout`、`stderr`、`exit_code`

### 命中审批路径

1. CLI 发起 `shell.run`
2. core 返回 `ShellRunOutcome::PendingApproval { approval_id }`
3. CLI 立即调用 `CoreControlMessage::ListPendingApprovals`
4. CLI 找到对应 `approval_id` 的待审批项并展示 summary
5. CLI 在终端提示用户确认：`Approve? [y/N]`
6. 用户输入 `y`/`yes`：CLI 调用 `ApprovePendingApproval`
7. 其他输入或空输入：CLI 调用 `RejectPendingApproval`
8. 若批准成功，CLI 携带同一 `approval_id` 重发原始 `shell.run`
9. CLI 输出最终执行结果

### 拒绝路径

- 用户拒绝后，CLI 不再重试 shell request
- CLI 直接以明确错误退出，例如 `approval rejected by user`

## [S4] 架构边界

### 保持短连接模型

CLI 继续沿用当前短连接请求模式：

- 一次 capability 请求一条连接
- 一次 core control 请求一条连接

不引入后台守护进程、长连接状态机或前端事件订阅。

### 协议复用

CLI 只消费现有两类协议：

- `kunkka.capability.v1`：`shell.run`
- `kunkka.core-control.v1`：`ListPendingApprovals`、`ApprovePendingApproval`、`RejectPendingApproval`

本设计不要求 core 新增 CLI 专属协议。

### 与其他前端的关系

- `kunkka-tui` 后续可复用同样的 approval control 消息
- 浏览器插件前端后续经 `kunkka-native-host` 消费同样消息

因此 CLI 只是 approval consumer 的第一刀，不是唯一入口。

## [S5] 输出与错误处理

### 成功输出

第一刀沿用纯文本输出，至少展示：

- `stdout`
- `stderr`
- `exit_code`

如果 `stdout` 非空，优先显示它；`stderr` 和退出码作为补充信息。

### 错误处理规则

- `permission_denied`：不进入审批交互，直接失败
- `PendingApproval` 后查不到对应 `approval_id`：视为 core 状态异常，直接失败
- approve/reject 请求失败：直接失败，不做自动重试
- 批准后重试若收到 `approval_denied`：直接透出，视为 approval 失效或已过期
- 用户明确拒绝：CLI 返回本地错误，不伪装成 core platform error

## [S6] 代码结构

```text
crates/kunkka-cli/src/
├── cli.rs         # shell 子命令参数定义
├── client.rs      # shell capability 调用 + approval control 调用
├── lib.rs         # shell 命令执行编排
├── output.rs      # shell 成功/失败输出格式
└── error.rs       # approval rejected / missing approval 等错误
```

最小实现可以继续把编排逻辑留在 `lib.rs`，不强行拆新模块；只有当逻辑明显变长时再抽 helper。

## [S7] 测试策略

### 单元测试

- CLI 参数解析：`shell --app ... --command ...`
- 用户确认输入归一化：`y/yes` 与默认拒绝

### 集成测试

- shell completed 直接输出结果
- shell pending -> approve -> retry success
- shell pending -> reject -> CLI 失败退出
- pending approval 查不到 / 已过期时，CLI 失败并输出明确错误

测试继续复用仓库现有模式：

- `tempdir + KunkkaPaths`
- 进程内 `prepare_core_runtime()`
- 必要时通过参数或可注入输入源模拟终端确认

### 验证命令

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
