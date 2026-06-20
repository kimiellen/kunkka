# SQLite 能力 (SQLite Capability)

## 概述

SQLite 能力允许 Worker 创建和管理自己的 SQLite 数据库。每个 App 拥有独立的数据库目录，支持通用 SQL 操作。

## 协议

- Schema: `kunkka.capability.v1`
- Capability: `sqlite`

## 方法

### open

打开数据库连接。如果数据库文件不存在则自动创建。

**请求参数：**

```rust
struct SqliteOpenParams {
    path: Option<String>,  // 数据库文件名（相对于 app-data/<app_id>/），默认 "app.db"
}
```

**响应结果：**

```rust
enum SqliteResponse {
    Opened {
        path: String,  // 数据库文件的绝对路径
    }
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `permission_denied` | 路径尝试逃逸 app-data 目录 |
| `connection_error` | 无法打开数据库 |

### query

执行查询 SQL（SELECT），返回结果集。

**请求参数：**

```rust
struct SqliteQueryParams {
    sql: String,               // SQL 语句
    params: Vec<Vec<u8>>,      // 参数列表（每个参数是 postcard 编码的 SqliteValue）
}
```

**响应结果：**

```rust
enum SqliteResponse {
    Queried {
        columns: Vec<String>,                    // 列名列表
        rows: Vec<Vec<Option<Vec<u8>>>>,         // 行数据
    }
}
```

- `columns` 是列名列表
- `rows` 是行数据，每行是列值列表
- 列值为 `None` 表示 NULL，`Some(bytes)` 是 SQLite 原始值的 postcard 编码

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `not_open` | 数据库未打开 |
| `invalid_params` | 参数解码失败 |
| `query_error` | SQL 执行错误 |

### execute

执行写入 SQL（INSERT/UPDATE/DELETE/DDL），返回影响行数。

**请求参数：**

```rust
struct SqliteExecuteParams {
    sql: String,               // SQL 语句
    params: Vec<Vec<u8>>,      // 参数列表
}
```

**响应结果：**

```rust
enum SqliteResponse {
    Executed {
        rows_affected: u64,  // 影响的行数
    }
}
```

**错误码：**

| 错误码 | 说明 |
|--------|------|
| `not_open` | 数据库未打开 |
| `invalid_params` | 参数解码失败 |
| `execute_error` | SQL 执行错误 |

### close

关闭数据库连接。

**请求参数：** 空

**响应结果：**

```rust
enum SqliteResponse {
    Closed
}
```

## 参数绑定

SQL 参数使用位置绑定（`?1`, `?2`, ...）。

参数值通过 postcard 编码为 `Vec<u8>`，支持以下类型：

```rust
enum SqliteValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
```

| Rust 类型 | SQLite 类型 |
|-----------|------------|
| `i64` | INTEGER |
| `f64` | REAL |
| `String` | TEXT |
| `Vec<u8>` | BLOB |
| `None` | NULL |

## 数据库存储位置

```
$XDG_DATA_HOME/kunkka/app-data/<app_id>/<db_name>
```

- 默认数据库名：`app.db`
- 目录自动创建
- 路径规范化防止目录遍历

## SQLite 配置

- `journal_mode = WAL`（强制）
- `synchronous = FULL`（强制）
- `foreign_keys = ON`（强制）
- 连接池大小：1

## Worker SDK 使用示例

### 打开数据库

```rust
use kunkka_worker_sdk::{call_capability, AppId};
use serde::{Deserialize, Serialize};

async fn open_db(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<String, Box<dyn std::error::Error>> {
    let params = SqliteOpenParams { path: None };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "sqlite",
        "open",
        params_bytes,
    ).await?;

    let result: SqliteResponse = postcard::from_bytes(&response)?;
    match result {
        SqliteResponse::Opened { path } => Ok(path),
        _ => Err("unexpected response".into()),
    }
}
```

### 执行查询

```rust
async fn query_users(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<Vec<User>, Box<dyn std::error::Error>> {
    let params = SqliteQueryParams {
        sql: "SELECT id, name, email FROM users WHERE active = ?1".to_string(),
        params: vec![
            postcard::to_stdvec(&SqliteValue::Integer(1))?,
        ],
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "sqlite",
        "query",
        params_bytes,
    ).await?;

    let result: SqliteResponse = postcard::from_bytes(&response)?;
    match result {
        SqliteResponse::Queried { columns, rows } => {
            // 解析行数据...
            Ok(users)
        }
        _ => Err("unexpected response".into()),
    }
}
```

### 插入数据

```rust
async fn insert_user(
    socket_path: &Path,
    app_id: &AppId,
    name: &str,
    email: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let params = SqliteExecuteParams {
        sql: "INSERT INTO users (name, email) VALUES (?1, ?2)".to_string(),
        params: vec![
            postcard::to_stdvec(&SqliteValue::Text(name.to_string()))?,
            postcard::to_stdvec(&SqliteValue::Text(email.to_string()))?,
        ],
    };
    let params_bytes = postcard::to_stdvec(&params)?;

    let response = call_capability(
        socket_path,
        app_id,
        "sqlite",
        "execute",
        params_bytes,
    ).await?;

    let result: SqliteResponse = postcard::from_bytes(&response)?;
    match result {
        SqliteResponse::Executed { rows_affected } => Ok(rows_affected),
        _ => Err("unexpected response".into()),
    }
}
```

### 关闭数据库

```rust
async fn close_db(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = SqliteCloseParams {};
    let params_bytes = postcard::to_stdvec(&params)?;

    call_capability(
        socket_path,
        app_id,
        "sqlite",
        "close",
        params_bytes,
    ).await?;

    Ok(())
}
```

## 安全注意事项

- 每个 App 的数据库目录隔离，无法访问其他 App 的数据库
- 路径规范化防止目录遍历攻击
- WAL 模式提供崩溃安全性
- synchronous=FULL 确保数据落盘
