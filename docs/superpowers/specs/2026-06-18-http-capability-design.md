# HTTP Capability (External API Request) 设计

## [S1] 概述

Kunkka 的下一刀 capability layer 引入 `http` capability，用于让 worker 请求 core 代发 HTTP 请求访问外部 API。第一刀目标是一个**域名白名单可控、透传原始 HTTP 语义**的最小 HTTP 代理能力。

本设计同时满足三件事：

1. worker 以完整 HTTP 请求形式提交（method、URL、headers、body）。
2. manifest 声明允许的域名白名单，域名不在白名单内直接拒绝。
3. core 使用 reqwest 发起 HTTP 请求，透传完整 HTTP 响应（status code、headers、body）。

这不是完整 HTTP 客户端平台。第一刀不支持 WebSocket、流式响应、代理、Cookie 管理或请求缓存。

## [S2] HTTP Capability 协议

Schema 仍使用 `kunkka.capability.v1`，新增 `capability = "http"` 与方法 `method = "request"`。

### 请求参数

```rust
struct HttpRequestParams {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}
```

- `method` 是 HTTP 方法，允许 `GET`、`POST`、`PUT`、`PATCH`、`DELETE`。
- `url` 是完整 URL，包含 scheme 和域名。
- `headers` 是请求头列表，key-value 对。
- `body` 是可选的请求体。

### 返回结果

`CapabilityResponse.result` 中的成功 bytes 编码为：

```rust
struct HttpResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
```

- `status_code` 是 HTTP 状态码。
- `headers` 是响应头列表，key-value 对。
- `body` 是响应体。

### 错误码

- `invalid_params`：参数解码失败、URL 无效或非法 HTTP 方法
- `permission_denied`：URL 域名不在白名单
- `timeout`：请求超时（30 秒）
- `io_error`：网络错误
- `scheme_not_allowed`：URL scheme 不是 http 或 https

## [S3] Manifest 配置

```json
{
  "capabilities": {
    "http": {
      "domains": ["api.github.com", "hooks.slack.com"]
    }
  }
}
```

### 类型定义

```rust
struct HttpCapabilityConfig {
    domains: Vec<String>,
}
```

### 验证规则

- `domains` 中的域名不能为空字符串。
- 域名精确匹配（不含子域名），如 `api.github.com` 不匹配 `gist.github.com`。
- URL 的 host 必须与白名单中的某个域名完全相等。

## [S4] 域名白名单匹配

白名单匹配逻辑：

1. 从请求 URL 中提取 host（不含端口）。
2. 与 `domains` 列表中的每个域名做精确字符串比较。
3. 大小写不敏感（域名标准行为）。
4. 匹配失败返回 `permission_denied`。

示例：

- 白名单 `[api.github.com]` → `https://api.github.com/repos` ✓
- 白名单 `[api.github.com]` → `https://gist.github.com/` ✗
- 白名单 `[api.github.com]` → `https://API.GITHUB.COM/` ✓

## [S5] HTTP 客户端行为

### 超时

固定 30 秒超时，不可配置。

### 重定向

自动跟随重定向，最多 10 次。重定向后的域名也必须在白名单内，否则返回 `permission_denied`。

### 压缩

自动处理 gzip/deflate 压缩。

### HTTP 版本

自动协商 HTTP/1.1 或 HTTP/2。

### TLS

使用系统默认 TLS 配置。

### 不支持

- WebSocket
- 流式响应
- 代理
- Cookie 管理
- 请求缓存
- 请求重试
- 请求队列
- HTTP/2 Server Push
- 请求取消

## [S6] 实现架构

### 新增文件

- `crates/kunkka-core/src/capability/http.rs` — HTTP capability handler

### 修改文件

- `crates/kunkka-core/src/capability/mod.rs` — 添加 `pub mod http` 和路由
- `crates/kunkka-core/src/app_manifest.rs` — 添加 `HttpCapabilityConfig` 类型
- `crates/kunkka-core/Cargo.toml` — 添加 `reqwest` 依赖

### 测试文件

- `crates/kunkka-core/tests/http_capability.rs` — HTTP capability 集成测试

## [S7] 安全考虑

- 域名白名单精确匹配，防止子域名绕过。
- 只允许 http 和 https scheme。
- 固定超时防止请求挂起。
- Worker 自行管理认证信息，core 不记录或转发敏感 header。
- 无日志记录，保护请求隐私。

## [S8] Worker SDK

`kunkka-worker-sdk` 中已有 `call_capability` 函数，无需修改。Worker 侧通过 `capability = "http"`、`method = "request"` 和 postcard 编码的 `HttpRequestParams` 使用此 capability。
