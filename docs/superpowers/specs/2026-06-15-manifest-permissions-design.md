# Manifest Frontend Dispatch Permissions 设计

## 状态

已在 2026-06-15 批准用于规格化。

## 背景

`kunkka-core` 当前的 frontend dispatch handler 通过 `allow_frontend_dispatch_v1()` 临时允许所有请求。该函数返回 `true`，不区分 app、method 或 subject，是一个需要替换的临时替换点。

本切片实现最小权限检查：在 app manifest 中声明哪些 method 允许 frontend dispatch，core 默认拒绝未声明的 dispatch。

## 目标

- 删除 `allow_frontend_dispatch_v1()` 临时函数。
- 在 app manifest 中支持 `permissions.frontend_dispatch.allowed_methods` 声明。
- 实现 core-owned deny-by-default frontend dispatch permission check。
- 保持权限决策在 `kunkka-core` 内部，不在 native-host 或 protocol 层做 allow/deny。
- 第一版保持 manifest 静态配置，不引入动态授权、DB 持久化或用户交互。

## 非目标

- 不实现完整 permission system。
- 不支持 wildcard、pattern、deny list 或 method group。
- 不支持 subject-level permission（区分 frontend form、CLI、TUI 等）。
- 不修改 `kunkka-ipc`、`kunkka-protocol`、`kunkka-worker-sdk`、`kunkka-native-host`。
- 不引入新的 IPC message 或 Native Messaging JSON command。
- 不支持运行时动态授权、grant/revoke。
- 不实现 permission audit log。

## 架构边界

`kunkka-core` 拥有 permission decision。本切片不修改任何 IPC 协议或 frontend/native-host 层。

```text
Browser Extension -> Native Messaging JSON -> kunkka-native-host
  -> Kunkka IPC frontend-dispatch
  -> kunkka-core: validate request -> lookup manifest -> check permissions -> dispatch_with_start
```

`kunkka-native-host` 不做 allow/deny 决策。Browser Extension 不持有本地 capability 权限。Worker invocation 权限判断和 worker lifecycle 都在 `kunkka-core` 内部。

## Manifest 权限模型

`permissions` 是 `AppManifest` 的可选字段。

Rust 概念模型：

```rust
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    pub permissions: AppPermissions,
    pub idle_timeout_ms: u64,
    pub startup_timeout_ms: u64,
}

pub struct AppPermissions {
    pub frontend_dispatch: FrontendDispatchPermissions,
}

pub struct FrontendDispatchPermissions {
    pub allowed_methods: Vec<String>,
}
```

JSON schema 规则：

```json
{
  "app_id": "notes",
  "worker": {
    "program": "/usr/bin/notes-worker",
    "args": ["--serve"]
  },
  "permissions": {
    "frontend_dispatch": {
      "allowed_methods": ["search", "open"]
    }
  }
}
```

`permissions` 在 JSON 中是可选字段。Rust 层面使用 `#[serde(default)]` 实现：`AppPermissions`、`FrontendDispatchPermissions` 和 `allowed_methods` 都有 `Default` impl，缺失时分别默认为空结构或空 Vec。

`permissions` 缺失：默认 `AppPermissions::default()`，拒绝所有 frontend dispatch。

`permissions.frontend_dispatch` 缺失：默认 `FrontendDispatchPermissions::default()`，拒绝所有 frontend dispatch。

`allowed_methods` 缺失或为空数组：拒绝所有 frontend dispatch。

`allowed_methods` 中出现空字符串或纯空白字符串：manifest invalid，加载失败。

## Permission Decision

权限判断放在 `crates/kunkka-core/src/permissions.rs`，保持很小：

```rust
pub enum PermissionDecision {
    Allow,
    Deny { code: &'static str, message: String },
}

pub fn decide_frontend_dispatch(
    manifest: &AppManifest,
    method: &str,
) -> PermissionDecision
```

判断语义：

```text
allowed = manifest.permissions.frontend_dispatch.allowed_methods contains request.method
```

- `allowed_methods` 包含 `request.method`：`Allow`。
- `allowed_methods` 不包含 `request.method`：`Deny { code: "permission_denied", message: "..." }`。

精确匹配规则：

- 只支持精确 method 名匹配，如 `"search"`。
- 不支持 wildcard、pattern 或 method alias。
- 匹配区分大小写。
- 匹配前不做 trim（method 已在 runtime handler 中校验非空）。

## Runtime 行为与错误处理

frontend dispatch 执行顺序改为：

```text
decode frontend-dispatch request
-> validate app_id non-empty
-> validate method non-empty
-> lookup app manifest
-> check manifest permissions via decide_frontend_dispatch
-> if Deny: return PlatformError { code: "permission_denied" }
-> dispatch_with_start
-> map worker/core result
```

关键行为：

- `app_id` 不存在：返回 `PlatformError { code: "app_not_found" }`。
- `method` 未授权：返回 `PlatformError { code: "permission_denied" }`。
- 未授权时不调用 `dispatch_with_start`，不会启动 cold worker。
- worker app error 仍保持 `AppError`，不混入权限错误。
- native-host 继续透传 platform error，不做权限判断。
- `permission_denied` message 包含 app_id 和 method，便于调试，不暴露本地路径或 secret。

`permission_denied` platform error 示例：

```json
{
  "id": "req-1",
  "ok": false,
  "error": {
    "code": "permission_denied",
    "message": "frontend dispatch method \"delete\" is not allowed for app \"notes\""
  }
}
```

## 测试策略

使用 TDD，按模块分层。

`app_manifest` tests：

- 加载含 `permissions.frontend_dispatch.allowed_methods` 的 manifest。
- 缺失 `permissions` 字段时，`allowed_methods` 默认为空（拒绝所有）。
- 缺失 `permissions.frontend_dispatch` 字段时，`allowed_methods` 默认为空。
- `allowed_methods` 包含空字符串或纯空白字符串时，manifest invalid。
- 现有 manifest tests 继续通过。

`permissions` tests：

- `allowed_methods` 包含目标 method：返回 `Allow`。
- `allowed_methods` 不包含目标 method：返回 `Deny`。
- `allowed_methods` 为空：返回 `Deny`。
- method 匹配区分大小写。
- method 匹配不做 trim。

`frontend_dispatch_runtime` tests：

- `allowed_methods` 包含 method 时，dispatch 正常路由到 worker。
- `allowed_methods` 不包含 method 时，返回 `PlatformError { code: "permission_denied" }`。
- 未授权时不会启动 cold worker。
- manifest 不存在时，仍返回 `PlatformError { code: "app_not_found" }`。
- 现有 frontend dispatch tests（empty app_id、empty method、app error、status then dispatch）继续通过。

native-host tests：

- 现有 native-host tests 不需要改协议，只需确认 platform error 仍透传。
- 如果现有 frontend dispatch integration test 写死了临时 allow 行为，需要更新 manifest fixture。

## 实施备注

建议实施顺序：

1. 在 `kunkka-core/src/app_manifest.rs` 添加 `AppPermissions` 和 `FrontendDispatchPermissions`，解析 manifest 中的可选 `permissions` 字段。
2. 在 `app_manifest` tests 中验证 `permissions` 的默认值、加载和校验。
3. 在 `kunkka-core/src/permissions.rs` 添加 `decide_frontend_dispatch` 和 `PermissionDecision`。
4. 在 `permissions` tests 中验证 `Allow` / `Deny` 语义。
5. 在 `runtime.rs` 删除 `allow_frontend_dispatch_v1()`，改为 manifest lookup + `decide_frontend_dispatch`。
6. 在 `frontend_dispatch_runtime` tests 中验证未授权返回 `permission_denied`、不会启动 cold worker。
7. 更新现有测试中需要 manifest fixture 的 case。
8. 更新 `docs/permissions.md`、`docs/architecture.md`、`docs/development-log.md`。
9. 运行 workspace fmt、test、clippy 验证。

## 文件变更范围

预期变更文件：

- `crates/kunkka-core/src/app_manifest.rs`：添加 `AppPermissions`、`FrontendDispatchPermissions`。
- `crates/kunkka-core/src/permissions.rs`：新增模块，`decide_frontend_dispatch` 和 `PermissionDecision`。
- `crates/kunkka-core/src/lib.rs`：添加 `pub mod permissions`。
- `crates/kunkka-core/src/runtime.rs`：删除 `allow_frontend_dispatch_v1()`，修改 `handle_frontend_dispatch_request`。
- `crates/kunkka-core/tests/app_manifest.rs`：添加 permissions 相关 tests。
- `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`：添加 permission denied tests，更新现有 fixture。
- `crates/kunkka-core/tests/permissions.rs`：新增 permission decision tests。
- `docs/permissions.md`：更新当前权限状态。
- `docs/architecture.md`：更新当前实现切片。
- `docs/development-log.md`：添加 manifest permissions 记录。
