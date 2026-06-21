# CLI Frontend 设计

> **注意**：本文档中的 "CLI frontend" 指 `kunkka-cli`，即 Kunkka 底座自身的管理命令行入口（`kunkka` 命令），不是基于 Kunkka 开发的上层应用 CLI 工具。

## 状态

已在 2026-06-16 批准用于规格化。

## 背景

Kunkka 已实现 IPC transport、core-control、worker dispatch、native-host bridge 和 manifest-based frontend dispatch permissions。下一步需要一个本地 CLI frontend，作为调试入口和验证 frontend dispatch / permission 链路的稳定手段。

Browser Extension 通过 `kunkka-native-host` 进入本地系统，CLI 通过 `kunkka-cli` 直接连接 core socket。两者都是 frontend form，都走 Kunkka IPC over Unix Domain Socket，都由 `kunkka-core` 执行权限判断。

## 目标

- 新增 `crates/kunkka-cli`，作为 Kunkka 的 CLI frontend。
- 第一版支持 `ping`、`status`、`dispatch` 三个命令。
- CLI 直接使用 `kunkka-protocol` 的 `core-control` 和 `frontend-dispatch` typed protocol。
- CLI dispatch 进入 `kunkka-core`，由 core 执行 manifest permissions。
- 输出默认为 JSON，便于脚本和测试。
- 保持 CLI 不自动启动 core、不读取 manifest、不判断权限、不直接调用 worker。

## 非目标

- 不实现 core 自动启动。
- 不实现 shell、文件、LLM、数据库等 capability 调用。
- 不实现 pretty/colored output、interactive mode。
- 不实现 stdin payload 读取。
- 不实现 stream、cancel、heartbeat。
- 不实现 generic IPC frame 透传。
- 不修改 `kunkka-ipc`、`kunkka-protocol`、`kunkka-worker-sdk`、`kunkka-native-host`、`kunkka-core`。

## 架构边界

CLI frontend 的职责：

```text
kunkka CLI args/input -> Kunkka IPC request -> kunkka-core -> response -> CLI JSON output
```

CLI 通过 `KunkkaPaths::resolve()` 找到 core socket。CLI 直接使用 `kunkka-protocol` 的 typed protocol。CLI 不连接 worker、不读取 manifest、不做权限判断。

CLI 与 native-host 的区别：

- native-host 桥接 Native Messaging JSON 和 Kunkka IPC。
- CLI 直接使用 Kunkka IPC typed protocol，无需 JSON envelope。
- CLI 输出的 JSON shape 可以与 native-host 不同，但 error code 保持一致。

## 命令语义

### kunkka ping

```text
kunkka ping
```

成功输出：

```json
{"ok":true,"result":{"type":"pong"}}
```

### kunkka status

```text
kunkka status
```

成功输出：

```json
{"ok":true,"result":{"type":"status","worker_count":0,"socket_path":"/path/to/core.sock","runtime_ready":true}}
```

### kunkka dispatch

```text
kunkka dispatch --app <app_id> --method <method> --payload <json>
```

成功输出：

```json
{"ok":true,"result":{"type":"dispatch","payload":{"items":[]}}}
```

Worker app error 输出：

```json
{"ok":true,"result":{"type":"dispatch_error","code":"not_found","message":"note not found"}}
```

Platform/core/IPC error 输出：

```json
{"ok":false,"error":{"code":"permission_denied","message":"..."}}
```

## 输入规则

- `--app` 必须非空 string。
- `--method` 必须非空 string。
- `--payload` 必须是合法 JSON value（object、array、string、number、boolean、null）。
- `dispatch` payload 作为 `application/json` opaque `Payload` 发送。
- `ping` 和 `status` 不需要 `--app`、`--method`、`--payload`。

## Exit Code

- `0`：CLI request 完成，即使是 worker app error。
- `1`：invalid CLI input、core unavailable、IPC/protocol/platform error。

Worker app error 仍是 app-level result，不等同平台失败。

## 输出 JSON Shape

成功：

```json
{"ok":true,"result":{...}}
```

失败：

```json
{"ok":false,"error":{"code":"...","message":"..."}}
```

`result.type` 值：

- `pong`：ping 成功。
- `status`：status 成功。
- `dispatch`：worker success payload。
- `dispatch_error`：worker app error。

`error.code` 值：

- `invalid_request`：CLI input 无效。
- `core_unavailable`：socket 不存在或连接失败。
- `core_ipc_error`：IPC send/recv/decode 失败。
- `unexpected_core_response`：response frame 或 protocol message 不符合预期。
- `permission_denied`、`app_not_found`、`worker_start_failed`、`worker_start_timeout`、`worker_unavailable`、`dispatch_ipc_error`、`unexpected_worker_response`、`core_error`：透传 core platform error。

## CLI Client 内部结构

```text
crates/kunkka-cli/
  Cargo.toml
  src/
    lib.rs
    main.rs
    cli.rs
    client.rs
    output.rs
    error.rs
  tests/
    cli_args.rs
    output.rs
    client_mapping.rs
```

职责：

- `cli.rs`：`clap` 参数定义和输入校验。
- `client.rs`：通过 UDS 连接 core，发送 `core-control` 或 `frontend-dispatch` request。
- `output.rs`：统一 JSON response schema。
- `error.rs`：CLI error code 与 exit code 映射。
- `main.rs`：薄入口，只负责调用 lib 并设置 process exit code。

## 测试策略

- 参数解析测试：`ping/status/dispatch` 成功解析；空 app/method、非法 JSON 失败。
- 输出测试：pong/status/dispatch/app error/platform error JSON shape。
- client mapping 测试：CLI command 映射到正确 protocol payload。
- 集成测试：启动 `kunkka-core` runtime，运行 CLI client session 测 `ping/status`。
- dispatch 集成测试：注册 warm worker + manifest permission，CLI dispatch 返回 worker JSON payload。
- 不做 shell-level CLI binary snapshot；优先测试 library 层，减少 brittle。

## 依赖

- 新增 workspace member `crates/kunkka-cli`。
- 使用 `clap`，需加入 workspace dependency。
- 使用现有 `tokio`、`serde`、`serde_json`、`kunkka-ipc`、`kunkka-protocol`、`kunkka-core`。

## 实施备注

建议实施顺序：

1. 在 workspace `Cargo.toml` 添加 `clap` dependency 和 `crates/kunkka-cli` member。
2. 创建 `crates/kunkka-cli` crate skeleton：`Cargo.toml`、`src/lib.rs`、`src/main.rs`。
3. 实现 `cli.rs`：`clap` 参数定义、输入校验、`CliCommand` enum。
4. 实现 `output.rs`：`CliOutput`、`CliErrorBody`、`CliResult` JSON types。
5. 实现 `error.rs`：`CliError`、exit code 映射。
6. 实现 `client.rs`：`CoreClient` session，发送 `core-control` 和 `frontend-dispatch` request。
7. 实现 `main.rs`：薄入口，连接 cli/client/output/error。
8. 添加参数解析 tests。
9. 添加输出 JSON shape tests。
10. 添加 client mapping tests。
11. 添加 `ping/status` 集成测试。
12. 添加 `dispatch` 集成测试。
13. 更新 `README.md`、`docs/architecture.md`、`docs/development-log.md`。
14. 运行 workspace fmt、test、clippy 验证。
