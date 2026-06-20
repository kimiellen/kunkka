# 文件系统能力 (FS Capability)

## 概述

文件系统能力允许 Worker 读取、写入文件和列出目录内容。所有文件操作都受到 App Manifest 中声明的路径白名单限制。

## 协议

- Schema: `kunkka.capability.v1`
- Capability: `fs`

## 方法

### read_file

读取文件内容（UTF-8 文本）。

**请求参数：**

```rust
struct ReadFileParams {
    path: String,  // 文件路径
}
```

**响应结果：**

```rust
struct ReadFileResult {
    content: String,  // 文件内容
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 路径不在白名单中 |
| `not_found` | 文件不存在 |
| `not_utf8` | 文件内容不是有效的 UTF-8 |
| `io_error` | 其他 IO 错误 |

### write_file

写入文本内容到文件。

**请求参数：**

```rust
struct WriteFileParams {
    path: String,     // 文件路径
    content: String,  // 要写入的内容
}
```

**响应结果：**

```rust
struct WriteFileResult {
    bytes_written: u64,  // 写入的字节数
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 路径不在白名单中 |
| `io_error` | 写入失败 |

### list_dir

列出目录内容。

**请求参数：**

```rust
struct ListDirParams {
    path: String,  // 目录路径
}
```

**响应结果：**

```rust
struct ListDirResult {
    entries: Vec<DirEntry>,
}

struct DirEntry {
    name: String,       // 文件/目录名
    entry_type: String, // "file"、"dir"、"symlink" 或 "other"
    size: u64,          // 文件大小（字节）
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 路径不在白名单中 |
| `not_found` | 目录不存在 |
| `io_error` | 读取失败 |

## 权限配置

在 App Manifest 中配置允许访问的路径：

```json
{
  "app_id": "my-app",
  "worker_program": "/path/to/worker",
  "capabilities": {
    "fs": {
      "paths": [
        "/home/user/documents/",
        "/home/user/.config/my-app/config.json"
      ]
    }
  }
}
```

### 路径匹配规则

1. **目录前缀匹配**：以 `/` 结尾的路径匹配该目录下的所有文件
   - `/home/user/documents/` 匹配 `/home/user/documents/file.txt`
   - 不匹配 `/home/user/documents`（缺少尾部斜杠）

2. **精确文件匹配**：不以 `/` 结尾的路径只匹配该精确文件
   - `/home/user/.config/my-app/config.json` 只匹配该文件

3. **路径规范化**：自动处理 `..` 和 `.` 等路径组件
   - `/home/user/../user/file.txt` 规范化为 `/home/user/file.txt`

## Worker SDK 使用示例

```rust
use kunkka_worker_sdk::{call_capability, AppId};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ReadFileParams {
    path: String,
}

#[derive(Deserialize)]
struct ReadFileResult {
    content: String,
}

async fn read_file(socket_path: &Path, app_id: &AppId, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let params = ReadFileParams {
        path: path.to_string(),
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "fs",
        "read_file",
        params_bytes,
    ).await?;

    let result: ReadFileResult = postcard::from_bytes(&response)?;
    Ok(result.content)
}
```

## 安全注意事项

- 路径白名单在 App Manifest 中声明，Core 会在每次请求时验证
- 路径规范化防止目录遍历攻击（如 `../../etc/passwd`）
- Worker 无法访问白名单之外的文件
