# Capability Layer 设计

## [S1] 概述

Kunkka 是本地能力平台，core 为 app worker 提供受控的本地能力访问。本设计实现 Capability Layer 的第一刀：文件系统能力（read_file、write_file、list_dir）。

Worker 通过独立的短连接向 core 发送能力请求（复用 core socket），core 校验权限后执行操作并返回结果。能力请求使用独立的 `kunkka.capability.v1` payload schema，与 dispatch 并行但不耦合。

## [S2] Capability 协议

Schema: `kunkka.capability.v1`

### 请求

Worker 发送 `Frame::Request`，payload 编码为：

```rust
struct CapabilityRequest {
    app_id: String,      // 发起请求的 app，用于查找 manifest
    capability: String,  // "fs"
    method: String,      // "read_file", "write_file", "list_dir"
    params: Vec<u8>,     // postcard-encoded method-specific params
}
```

### 响应

Core 返回 `Frame::Response`，payload 编码为：

```rust
struct CapabilityResponse {
    result: Result<Vec<u8>, CapabilityError>,
}

struct CapabilityError {
    code: String,     // "permission_denied", "not_found", "io_error", etc.
    message: String,
}
```

`result` 中的 `Vec<u8>` 是 postcard-encoded 的方法特定返回值。

### 连接模型

Worker 注册后的连接由 core 主导（core 发 dispatch，worker 回复），worker 无法主动发送请求。因此 capability 请求使用**独立连接**：

1. Worker 向 core socket 发起新连接
2. 第一帧为 capability request（schema = `kunkka.capability.v1`）
3. Core 识别 schema，进入 capability handler
4. Core 通过请求中的 `app_id` 字段查找 manifest 做权限校验
5. Core 执行操作，返回 response
6. 连接关闭（短连接模式，与 frontend 行为一致）

Core runtime 的 `run_connection()` 新增 `CAPABILITY_SCHEMA` 分支，处理流程与 frontend dispatch 类似但独立。

### Worker SDK 辅助

`kunkka-worker-sdk` 提供 capability 客户端辅助函数，worker 调用时内部建立短连接：

```rust
// SDK 辅助（kunkka-worker-sdk 内部）
// 内部建立短连接，设置 request.app_id，发送请求，返回响应
pub async fn call_capability(
    socket_path: &Path,
    app_id: &AppId,
    capability: &str,
    method: &str,
    params: Vec<u8>,
) -> Result<CapabilityResponse, Error>;
```

这与 `kunkka-cli` 和 `kunkka-native-host` 的 frontend 连接模式一致。

## [S3] 文件系统操作

### read_file

**参数：**
```rust
struct ReadFileParams {
    path: String,
}
```

**返回：**
```rust
struct ReadFileResult {
    content: String,  // UTF-8 文本
}
```

**错误码：**
- `permission_denied` — 路径不在白名单中
- `not_found` — 文件不存在
- `io_error` — 读取失败
- `not_utf8` — 文件内容不是有效 UTF-8

### write_file

**参数：**
```rust
struct WriteFileParams {
    path: String,
    content: String,  // UTF-8 文本
}
```

**返回：**
```rust
struct WriteFileResult {
    bytes_written: u64,
}
```

**错误码：**
- `permission_denied` — 路径不在白名单中
- `io_error` — 写入失败

### list_dir

**参数：**
```rust
struct ListDirParams {
    path: String,
}
```

**返回：**
```rust
struct ListDirResult {
    entries: Vec<DirEntry>,
}

struct DirEntry {
    name: String,
    entry_type: String,  // "file", "dir", "symlink"
    size: u64,
}
```

**错误码：**
- `permission_denied` — 路径不在白名单中
- `not_found` — 目录不存在
- `io_error` — 读取失败

## [S4] Manifest 权限

App manifest 新增 `capabilities` 字段：

```json
{
  "app_id": "notes",
  "worker": { "program": "notes-worker" },
  "permissions": {
    "frontend_dispatch": { "allowed_methods": ["search"] }
  },
  "capabilities": {
    "fs": {
      "paths": [
        "/home/user/notes/",
        "/tmp/notes-export.txt"
      ]
    }
  }
}
```

### 路径匹配规则

- 路径必须是绝对路径（以 `/` 开头）
- 以 `/` 结尾：目录前缀匹配，允许访问该目录及其子目录下的所有文件
- 不以 `/` 结尾：精确文件匹配，只允许访问该文件
- 路径在匹配前进行规范化（去除 `.`、`..`、连续 `/`）
- `write_file` 的目标路径必须匹配白名单
- `read_file` 的目标路径必须匹配白名单
- `list_dir` 的目标路径必须匹配白名单（以 `/` 结尾的条目允许 list 其本身）

### 缺失配置

- 缺少 `capabilities` 字段 = 禁止所有能力调用
- 缺少 `capabilities.fs` = 禁止所有文件系统操作
- `capabilities.fs.paths` 为空列表 = 禁止所有文件系统操作

## [S5] Core 内部架构

### 模块结构

```text
crates/kunkka-core/src/
├── capability/
│   ├── mod.rs           # capability 请求路由、协议类型
│   ├── fs.rs            # 文件系统操作实现
│   └── permissions.rs   # 路径白名单校验
├── runtime.rs           # 新增 capability 分发分支
└── app_manifest.rs      # 新增 capabilities 字段
```

### 协议类型（capability/mod.rs）

```rust
use serde::{Deserialize, Serialize};

pub const CAPABILITY_SCHEMA: &str = "kunkka.capability.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub app_id: String,
    pub capability: String,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub result: Result<Vec<u8>, CapabilityError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityError {
    pub code: String,
    pub message: String,
}
```

### 路径校验（capability/permissions.rs）

```rust
pub fn check_fs_permission(
    manifest: &AppManifest,
    path: &str,
) -> Result<(), CapabilityError> {
    // 1. 检查 capabilities.fs 是否存在
    // 2. 规范化请求路径
    // 3. 遍历 paths 白名单，按规则匹配
    // 4. 匹配成功返回 Ok，否则返回 permission_denied
}
```

### 文件系统实现（capability/fs.rs）

```rust
pub async fn handle_fs_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, CapabilityError> {
    match method {
        "read_file" => { /* decode params, check permission, read file, encode result */ }
        "write_file" => { /* decode params, check permission, write file, encode result */ }
        "list_dir" => { /* decode params, check permission, list dir, encode result */ }
        _ => Err(CapabilityError { code: "unknown_method".into(), message: ... }),
    }
}
```

### Runtime 分发（runtime.rs 修改）

`run_connection()` 新增 `CAPABILITY_SCHEMA` 分支：

```rust
match frame_schema(&first_frame) {
    Some(WORKER_PROTOCOL_SCHEMA) => { /* worker registration */ }
    Some(CORE_CONTROL_SCHEMA | FRONTEND_DISPATCH_SCHEMA) => { /* frontend */ }
    Some(CAPABILITY_SCHEMA) => { /* capability request, short-lived connection */ }
    ...
}
```

Capability 请求是短连接：core 读取第一帧，处理，返回响应，连接结束。请求中携带 `app_id` 字段用于查找 manifest 做权限校验（不需要从连接状态反查 worker）。

## [S6] 测试策略

### 单元测试

- `capability/permissions.rs`：路径规范化、前缀匹配、精确匹配、缺失配置拒绝
- `capability/fs.rs`：各操作的参数解码、结果编码

### 集成测试

- `tests/capability_fs.rs`：
  - 使用 `test_paths()` + `tempdir` + `KunkkaPaths` 模式
  - 启动 core runtime
  - 注册 mock worker（有 fs 权限的 manifest）
  - 通过 IPC 发送 capability request 测试 read_file、write_file、list_dir
  - 验证权限拒绝场景（路径不在白名单）
  - 验证缺失 capabilities 配置时的拒绝行为

### 验证命令

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
