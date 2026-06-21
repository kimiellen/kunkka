# SQLite Capability 设计

## [S1] 概述

Kunkka 的下一刀 capability layer 引入 `sqlite` capability，用于让 worker 创建和管理自己的 SQLite 数据库文件。第一刀目标是一个**单数据库、短连接、通用 SQL 操作**的最小数据库能力。

本设计同时满足三件事：

1. 每个 App 自动拥有 sqlite capability，无需 manifest 声明。
2. 数据库文件存放在 App 隔离目录，固定文件名 `app.db`。
3. 支持通用 SQL 操作（query/execute），参数绑定，返回列名和行数据。

这不是完整数据库管理平台。第一刀不支持连接池、多数据库、备份、vacuum 或 JSON 函数。

## [S2] SQLite Capability 协议

Schema 仍使用 `kunkka.capability.v1`，新增 `capability = "sqlite"` 与四个 method。

### Method: `open`

打开数据库连接。如果数据库文件不存在则自动创建。

**请求参数：** 空（无参数）

**返回结果：**
```rust
struct SqliteOpened {
    path: String,  // 数据库文件的绝对路径
}
```

### Method: `query`

执行查询 SQL（SELECT），返回结果集。

**请求参数：**
```rust
struct SqliteQueryParams {
    sql: String,
    params: Vec<Vec<u8>>,  // 位置参数，postcard 编码
}
```

**返回结果：**
```rust
struct SqliteQueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<Option<Vec<u8>>>>,
}
```

- `columns` 是列名列表
- `rows` 是行数据，每行是列值列表
- 列值为 `None` 表示 NULL，`Some(bytes)` 是 SQLite 原始值的 postcard 编码

### Method: `execute`

执行写入 SQL（INSERT/UPDATE/DELETE/DDL），返回影响行数。

**请求参数：**
```rust
struct SqliteExecuteParams {
    sql: String,
    params: Vec<Vec<u8>>,
}
```

**返回结果：**
```rust
struct SqliteExecuteResult {
    rows_affected: u64,
}
```

### Method: `close`

关闭数据库连接。

**请求参数：** 空

**返回结果：** 空

### 错误码

- `invalid_params` — 参数解码失败或 SQL 语法错误
- `database_error` — SQLite 执行错误（约束冲突、权限等）
- `io_error` — 文件系统错误（创建目录失败等）
- `not_open` — 数据库未打开时执行 query/execute/close

## [S3] 数据库配置

### 存储位置

```
$XDG_DATA_HOME/kunkka/app-data/<app_id>/app.db
```

- `app_id` 从 `CapabilityRequest.app_id` 获取
- 目录自动创建
- 文件名固定为 `app.db`

### SQLite Pragmas

- `journal_mode = WAL`（强制）
- `synchronous = FULL`（强制）
- `foreign_keys = ON`

### 连接管理

- 短连接模型：每次 open 创建新连接，close 关闭连接
- 不支持连接池
- 一个 App 同时只能有一个打开的连接

## [S4] 参数绑定

SQL 参数使用位置绑定（`?1`, `?2`, ...）。

参数值通过 postcard 编码为 `Vec<u8>`，支持以下类型映射：

| Rust 类型 | SQLite 类型 |
|-----------|------------|
| `i64` | INTEGER |
| `f64` | REAL |
| `String` | TEXT |
| `Vec<u8>` | BLOB |
| `None` | NULL |

App 负责将复杂数据结构序列化为 TEXT（JSON）或 BLOB（postcard）。

## [S5] 实现架构

### 新增文件

- `crates/kunkka-core/src/capability/sqlite.rs` — SQLite capability handler

### 修改文件

- `crates/kunkka-core/src/capability/mod.rs` — 添加 `pub mod sqlite` 和路由
- `crates/kunkka-core/Cargo.toml` — 无需修改（sqlx 已在依赖中）

### 测试文件

- `crates/kunkka-core/tests/sqlite_capability.rs` — SQLite capability 测试

## [S6] 安全考虑

- 数据库文件存放在 App 隔离目录，App 无法访问其他 App 的数据库
- 无 SQL 限制，完全信任 App（与 fs/shell capability 一致）
- synchronous=FULL 确保数据落盘
- WAL 模式提供崩溃安全性

## [S7] Worker SDK

`kunkka-worker-sdk` 中已有 `call_capability` 函数，无需修改。Worker 侧通过 `capability = "sqlite"` 和 postcard 编码的参数使用此 capability。
