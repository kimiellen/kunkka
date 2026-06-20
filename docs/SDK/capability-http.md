# HTTP 能力 (HTTP Capability)

## 概述

HTTP 能力允许 Worker 发起 HTTP 请求访问外部 API。所有请求都受到 App Manifest 中声明的域名白名单限制。

## 协议

- Schema: `kunkka.capability.v1`
- Capability: `http`

## 方法

### request

发起 HTTP 请求。

**请求参数：**

```rust
struct HttpRequestParams {
    method: String,              // HTTP 方法（GET、POST、PUT、PATCH、DELETE）
    url: String,                 // 完整 URL
    headers: Vec<(String, String)>,  // 请求头
    body: Option<Vec<u8>>,       // 请求体（可选）
}
```

**响应结果：**

```rust
struct HttpResponse {
    status_code: u16,                // HTTP 状态码
    headers: Vec<(String, String)>,  // 响应头
    body: Vec<u8>,                   // 响应体
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 域名不在白名单中或未配置 HTTP 能力 |
| `scheme_not_allowed` | URL 协议不是 http 或 https |
| `invalid_params` | URL 格式错误或 HTTP 方法不支持 |
| `timeout` | 请求超时（默认 30 秒） |
| `io_error` | 网络错误 |

## 权限配置

在 App Manifest 中配置允许访问的域名：

```json
{
  "app_id": "my-app",
  "worker_program": "/path/to/worker",
  "capabilities": {
    "http": {
      "domains": [
        "api.github.com",
        "api.openai.com"
      ]
    }
  }
}
```

### 域名匹配规则

1. **精确匹配**：域名比较不区分大小写
2. **重定向限制**：重定向目标域名也必须在白名单中
3. **最大重定向次数**：10 次

## 请求限制

| 限制项 | 值 |
|--------|-----|
| 超时时间 | 30 秒 |
| 最大重定向次数 | 10 |
| 允许的 HTTP 方法 | GET、POST、PUT、PATCH、DELETE |
| 允许的 URL 协议 | http、https |
| 代理 | 不使用系统代理 |

## Worker SDK 使用示例

```rust
use kunkka_worker_sdk::{call_capability, AppId};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct HttpRequestParams {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct HttpResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn http_get(
    socket_path: &Path,
    app_id: &AppId,
    url: &str,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let params = HttpRequestParams {
        method: "GET".to_string(),
        url: url.to_string(),
        headers: vec![],
        body: None,
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "http",
        "request",
        params_bytes,
    ).await?;

    let result: HttpResponse = postcard::from_bytes(&response)?;
    Ok(result)
}
```

### POST 请求示例

```rust
async fn http_post_json(
    socket_path: &Path,
    app_id: &AppId,
    url: &str,
    json_body: &str,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let params = HttpRequestParams {
        method: "POST".to_string(),
        url: url.to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
        ],
        body: Some(json_body.as_bytes().to_vec()),
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "http",
        "request",
        params_bytes,
    ).await?;

    let result: HttpResponse = postcard::from_bytes(&response)?;
    Ok(result)
}
```

## 安全注意事项

- 域名白名单在 App Manifest 中声明，Core 会在每次请求时验证
- 重定向目标也受白名单限制，防止通过重定向绕过限制
- 不使用系统代理，防止代理绕过域名限制
- 请求超时防止长时间阻塞
