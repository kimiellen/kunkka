# Shell Capability Approval 设计

## [S1] 概述

Kunkka 的下一刀 capability layer 引入 `shell` capability，用于让 worker 请求 core 代执行 shell 命令。第一刀目标不是完整 shell 平台，而是一个**可审批、可审计、支持简单管道**的最小执行能力。

本设计同时满足三件事：

1. worker 仍以字符串形式提交命令，支持 `cmd1 | cmd2` 这种简单管道。
2. manifest 可以声明哪些命令 `allow`、哪些命令 `ask`、其余命令 `deny`。
3. `ask` 命中时，core 生成待审批请求，由前端通过独立协议查询并做 approve/reject。

这不是完整真实 shell 语义。为了让白名单和审批规则可验证，第一刀只支持一个受限 shell 子集。

## [S2] Shell Capability 协议

Schema 仍使用 `kunkka.capability.v1`，只新增 `capability = "shell"` 与方法 `method = "run"`。

### 请求参数

```rust
struct ShellRunParams {
    command: String,
    approval_id: Option<String>,
}
```

- `command` 是 worker 请求执行的字符串命令。
- 首次执行时 `approval_id = None`。
- 命中 `ask` 并被前端批准后，worker 使用同一 `approval_id` 重试同一条命令。

### 返回结果

`CapabilityResponse.result` 中的成功 bytes 编码为：

```rust
enum ShellRunOutcome {
    Completed(ShellRunResult),
    PendingApproval(PendingApprovalReceipt),
}

struct ShellRunResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

struct PendingApprovalReceipt {
    approval_id: String,
}
```

### 错误码

- `invalid_params`：参数解码失败或语法不在受限子集内
- `permission_denied`：命中 `deny` 规则
- `approval_denied`：审批被拒绝或已过期
- `approval_mismatch`：`approval_id` 与命令内容不匹配
- `io_error`：shell 进程启动或等待失败

## [S3] 受限 Shell 子集

为了让命令白名单和审批模型可靠，第一刀只支持受限 shell 子集。

### 支持的语法

- 简单命令
- 顶层 `|` 管道
- 常规参数分词
- 常规单引号/双引号引用

### 明确拒绝的语法

- `&&`、`||`、`;`
- 重定向（`>`, `>>`, `<`）
- 命令替换（`$()`, 反引号）
- 分组 / 子 shell（`()`, `{}`）
- 前置环境变量赋值（`FOO=1 cmd`）
- 其他会让顶层命令提取变得不可靠的 shell 结构

### 解析与匹配

core 先把输入字符串解析为顶层 pipeline stages，再提取每个 stage 的命令名。manifest 规则匹配对象是这些**命令名**，不是完整字符串模板。

这意味着：

- `rg todo src | wc -l` 会提取命令名 `rg`, `wc`
- `echo 'a|b' | rg a` 中引号内的 `|` 不会被当作顶层管道

## [S4] Manifest Shell Policy

App manifest 在 `capabilities` 下新增 `shell` 配置：

```json
{
  "app_id": "notes",
  "worker": { "program": "notes-worker" },
  "capabilities": {
    "shell": {
      "allow": ["rg", "wc"],
      "ask": ["curl"]
    }
  }
}
```

### 规则语义

- `allow`：命令名命中时可直接执行
- `ask`：命令名命中时必须进入审批流
- 其余未命中的命令：`deny`

### 规则聚合

一条管道中只要有任一命令命中 `deny`，整条命令拒绝。

若没有 `deny`，但至少一个命令命中 `ask`，整条命令进入审批流。

只有当所有命令都命中 `allow` 时，整条命令直接执行。

### 缺失配置

- 缺少 `capabilities.shell` = 禁止所有 shell capability 请求
- `allow` 与 `ask` 都为空 = 禁止所有 shell capability 请求
- 同一个命令名不得同时出现在 `allow` 和 `ask`

## [S5] Approval 协议

审批是前端无关的 core 协议层，CLI、TUI、浏览器插件前端后续都通过它接入。浏览器插件路径为 `browser UI -> kunkka-native-host -> core`。

第一刀把审批控制加入 `kunkka.core-control.v1`，放在 `kunkka-protocol` 中扩展现有 `CoreControlMessage`。

### 新增消息

```rust
struct CoreListApprovalsRequest;

struct CoreListApprovalsResponse {
    approvals: Vec<PendingApproval>,
}

struct PendingApproval {
    approval_id: String,
    app_id: String,
    capability: String,
    summary: String,
}

struct CoreApproveApprovalRequest {
    approval_id: String,
}

struct CoreRejectApprovalRequest {
    approval_id: String,
}

struct CoreApprovalDecisionResponse;
```

### 待审批状态

core 在内存里维护 pending approvals。待审批项至少保存：

- `approval_id`
- `app_id`
- 原始 `command`
- 解析出的命令名列表
- 创建时间 `created_at`
- 当前状态（pending / approved / rejected / expired）

### 过期策略

- `Pending` approval 的 TTL 固定为 1 分钟
- TTL 只作用于 `Pending`，不作用于已 `Approved` 的 approval
- `Approved` approval 继续保持一次性消费模型：worker 成功携带 `approval_id` 重试后移除
- `Rejected` 与 `Expired` approval 不要求长期保留，可在后续访问中被清理

### 审批消费模型

1. worker 首次调用 `shell.run`
2. 命中 `ask` 时，core 创建 pending approval，返回 `PendingApprovalReceipt`
3. 前端通过 core control 查询待审批项
4. 前端调用 approve 或 reject
5. worker 以相同 `command` + `approval_id` 重试 `shell.run`
6. core 校验该审批已批准且命令内容匹配，再真正执行

### 无前端时的默认拒绝

本设计不要求 core 预先知道“当前有没有前端在线”。默认拒绝体现在：

- 未被批准的 `approval_id` 不能执行
- pending approval 在 1 分钟后超时过期
- 过期后重试返回 `approval_denied`

因此没有任何前端处理时，请求最终只能失败，不能静默放行。

### 懒清理

第一刀不引入 runtime 后台定时回收。approval store 在以下入口做懒清理：

- `list_pending`
- `approve`
- `reject`
- `consume_approved`

这些入口访问前，store 先扫描并处理已超时的 `Pending` 项：

- 超时的 `Pending` 变为 `Expired`
- 已经 `Rejected` 或 `Expired` 的旧项可在同轮访问中移除

这样可以保证：

- `list_pending` 不会返回超时项
- 对超时 approval 的 approve/reject/consume 都会失败
- 不需要在 runtime 主循环里增加新的周期任务

## [S6] Core 内部架构

### 模块结构

```text
crates/kunkka-core/src/
├── capability/
│   ├── mod.rs              # 扩展 shell capability 路由
│   ├── fs.rs
│   ├── shell.rs            # shell 参数、解析、执行、审批接入
│   └── permissions.rs      # 扩展 shell policy 决策
├── approval.rs             # pending approval 存储与决策
├── runtime.rs              # 扩展 core control 审批消息处理
└── app_manifest.rs         # 新增 capabilities.shell
```

`kunkka-protocol` 扩展 `core_control.rs`，承载 approval control 消息类型。

### 决策模型

`kunkka-core` 当前 `PermissionDecision` 只有 `Allow` / `Deny`。shell capability 需要扩展为三态，例如：

```rust
enum ShellPolicyDecision {
    Allow,
    Ask,
    Deny,
}
```

frontend dispatch 继续保留现有静态 allow/deny，不与 shell policy 混用。

### 执行模型

- shell 仍由 core 代执行
- 执行前先完成受限解析、policy 判定、必要时审批检查
- 审批通过后，core 再把原始命令字符串交给固定默认 shell 执行
- 返回 stdout、stderr、exit_code
- approval 过期与回收由 approval store 自身在访问路径上完成，不依赖 runtime 周期任务

## [S7] 测试策略

### 单元测试

- 受限 shell 子集解析：合法管道、引号中的 `|`、非法语法拒绝
- manifest shell policy 解析与校验
- shell policy 聚合：allow / ask / deny
- approval 存储：创建、查询、批准、拒绝、过期、懒清理

### 集成测试

- `allow` 命中时执行 `echo foo | wc -c` 并返回结果
- `deny` 命中时直接失败
- `ask` 命中时首次返回 `PendingApprovalReceipt`
- `approve` 后带 `approval_id` 重试成功
- `reject` 或过期后带 `approval_id` 重试失败
- core control 协议可列出并处理 pending approvals
- 过期后的 approval 不会出现在 pending list 中

### 验证命令

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
