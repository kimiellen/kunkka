# Shell 能力 (Shell Capability)

## 概述

Shell 能力允许 Worker 执行 shell 命令。支持三种权限策略：允许（Allow）、询问（Ask）和拒绝（Deny）。

## 协议

- Schema: `kunkka.capability.v1`
- Capability: `shell`

## 方法

### run

执行 shell 命令。

**请求参数：**

```rust
struct ShellRunParams {
    command: String,                // 要执行的命令
    approval_id: Option<String>,   // 审批 ID（用于询问策略的二次请求）
}
```

**响应结果：**

```rust
enum ShellRunOutcome {
    // 命令执行完成
    Completed(ShellRunResult),
    // 需要审批
    PendingApproval(PendingApprovalReceipt),
}

struct ShellRunResult {
    stdout: String,   // 标准输出
    stderr: String,   // 标准错误
    exit_code: i32,   // 退出码
}

struct PendingApprovalReceipt {
    approval_id: String,  // 审批 ID
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 命令不在允许列表中 |
| `invalid_params` | 命令格式错误 |
| `approval_mismatch` | 审批 ID 与命令不匹配 |
| `approval_denied` | 审批被拒绝或已过期 |
| `io_error` | 命令执行失败 |

## 权限配置

在 App Manifest 中配置 shell 权限：

```json
{
  "app_id": "my-app",
  "worker_program": "/path/to/worker",
  "capabilities": {
    "shell": {
      "allow": [
        "ls",
        "cat",
        "grep"
      ],
      "ask": [
        "rm",
        "mv",
        "cp"
      ]
    }
  }
}
```

### 权限策略

| 策略 | 说明 |
|------|------|
| `allow` | 命令直接执行，无需审批 |
| `ask` | 命令需要用户审批才能执行 |
| 不在列表中 | 命令被拒绝 |

### 策略决策逻辑

1. 如果命令在 `allow` 列表中 → 直接执行
2. 如果命令在 `ask` 列表中 → 返回 `PendingApproval`，等待用户审批
3. 如果命令不在任何列表中 → 拒绝

### 管道命令

支持管道命令（`|`），每个阶段的命令都会被检查：

- `ls | grep foo` → 检查 `ls` 和 `grep`
- 如果任一命令在 `ask` 列表中，整个管道需要审批
- 如果任一命令被拒绝，整个管道被拒绝

## 命令语法限制

- 支持：管道（`|`）
- 不支持：重定向（`>`、`<`）、逻辑运算符（`&&`、`||`）、后台执行（`&`）、命令替换（`$()`）、环境变量赋值

## 审批流程

当命令需要审批时，流程如下：

```
Worker                    Core                      Frontend
  |                         |                          |
  |--- run command -------->|                          |
  |                         |--- PendingApproval ----->|
  |                         |                          |--- 显示审批请求
  |                         |                          |<-- 用户批准
  |                         |<-- approve(approval_id) -|
  |<-- PendingApproval -----|                          |
  |                         |                          |
  |--- run command -------->|                          |
  |    (with approval_id)   |                          |
  |<-- Completed -----------|                          |
```

## Worker SDK 使用示例

### 直接执行命令

```rust
use kunkka_worker_sdk::{call_capability, AppId};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ShellRunParams {
    command: String,
    approval_id: Option<String>,
}

#[derive(Deserialize)]
enum ShellRunOutcome {
    Completed(ShellRunResult),
    PendingApproval(PendingApprovalReceipt),
}

#[derive(Deserialize)]
struct ShellRunResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Deserialize)]
struct PendingApprovalReceipt {
    approval_id: String,
}

async fn run_command(
    socket_path: &Path,
    app_id: &AppId,
    command: &str,
) -> Result<ShellRunOutcome, Box<dyn std::error::Error>> {
    let params = ShellRunParams {
        command: command.to_string(),
        approval_id: None,
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "shell",
        "run",
        params_bytes,
    ).await?;

    let result: ShellRunOutcome = postcard::from_bytes(&response)?;
    Ok(result)
}
```

### 处理审批流程

```rust
async fn run_command_with_approval(
    socket_path: &Path,
    app_id: &AppId,
    command: &str,
) -> Result<ShellRunResult, Box<dyn std::error::Error>> {
    // 第一次尝试执行
    let outcome = run_command(socket_path, app_id, command).await?;

    match outcome {
        ShellRunOutcome::Completed(result) => Ok(result),
        ShellRunOutcome::PendingApproval(receipt) => {
            // 等待用户审批（通过 Frontend）
            // 用户审批后，再次请求并携带 approval_id
            let params = ShellRunParams {
                command: command.to_string(),
                approval_id: Some(receipt.approval_id),
            };
            let params_bytes = postcard::to_stdvec(&params)?;

            let response = call_capability(
                socket_path,
                app_id,
                "shell",
                "run",
                params_bytes,
            ).await?;

            let result: ShellRunOutcome = postcard::from_bytes(&response)?;
            match result {
                ShellRunOutcome::Completed(result) => Ok(result),
                ShellRunOutcome::PendingApproval(_) => {
                    Err("unexpected pending approval".into())
                }
            }
        }
    }
}
```

## 安全注意事项

- 命令白名单在 App Manifest 中声明
- 管道中的每个命令都会被单独检查
- 不支持危险的 shell 语法（重定向、命令替换等）
- 审批机制确保敏感命令需要用户确认
