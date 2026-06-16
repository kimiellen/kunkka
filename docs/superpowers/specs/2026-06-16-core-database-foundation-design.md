# Core SQLite Database Foundation 设计

## 状态

已在 2026-06-16 批准用于规格化。

## 背景

Kunkka 已实现 IPC transport、core-control、worker dispatch、native-host bridge、manifest-based frontend dispatch permissions 和 CLI frontend。架构文档已指定 SQLite + sqlx 作为存储技术栈，`KunkkaPaths.database_path` 已指向 `$XDG_DATA_HOME/kunkka/kunkka.db`，但当前 workspace 没有 sqlx 依赖或数据库模块。

下一阶段需要建立 core-owned SQLite 基础层，为后续 permission audit、permission grant、app state 等持久化功能做准备。

## 目标

- 在 `kunkka-core` 新增 `database` 模块。
- 使用 SQLite + sqlx，连接 `KunkkaPaths.database_path`。
- `CoreRuntime::prepare()` 时初始化数据库连接并运行 migrations。
- 第一版只创建基础 metadata 表，验证数据库可打开、迁移可运行、schema version 可查询。
- 不改变现有 IPC、worker dispatch、frontend dispatch、CLI/native-host 行为。

## 非目标

- 不实现 permission audit 写入。
- 不实现 permission grant 持久化。
- 不实现 app-scoped database。
- 不实现 worker-facing database capability。
- 不在 CLI/native-host 直接访问数据库。
- 不新增单独 `kunkka-db` crate；第一版保持在 `kunkka-core` 内。
- 不修改 `kunkka-ipc`、`kunkka-protocol`、`kunkka-worker-sdk`、`kunkka-native-host`、`kunkka-cli`。

## 架构边界

`kunkka-core` 拥有 core database。第一版不暴露 DB 给 frontend protocol 或 worker protocol。

```text
CoreRuntime::prepare(paths)
  -> paths.ensure_dirs()
  -> CoreDatabase::connect(paths)
  -> CoreIpcServer::bind(paths)
  -> AppRegistry::load(paths)
  -> WorkerManager::with_app_registry(...)
```

DB 初始化失败时 core 不进入 ready 状态，不会绑定 socket。

## 数据库模块

`crates/kunkka-core/src/database.rs` 负责 core-owned DB 基础设施：

```rust
pub struct CoreDatabase {
    pool: sqlx::SqlitePool,
}

impl CoreDatabase {
    pub async fn connect(paths: &KunkkaPaths) -> Result<Self>;
    pub async fn schema_version(&self) -> Result<i64>;
    pub async fn ping(&self) -> Result<()>;
    pub fn pool(&self) -> &sqlx::SqlitePool;
}
```

- `connect()`：确保父目录存在、打开 SQLite 连接池、设置 pragmas、运行内嵌 migrations。
- `schema_version()`：从 `core_metadata` 查询 `schema_version` 并解析为 `i64`。
- `ping()`：执行 `SELECT 1`，验证连接可用。
- `pool()`：返回 `&sqlx::SqlitePool`，供后续模块使用。

## SQLite Pragmas

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
```

## Migration

使用 `sqlx::migrate!()` 编译期内嵌 migrations。

目录：

```text
crates/kunkka-core/migrations/
  0001_core_metadata.sql
```

第一版 migration：

```sql
CREATE TABLE IF NOT EXISTS core_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

INSERT INTO core_metadata (key, value)
VALUES ('schema_version', '1')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
```

迁移幂等：`CREATE TABLE IF NOT EXISTS` + `ON CONFLICT DO UPDATE`。

## 错误模型

`CoreError` 新增 variant：

```rust
#[error("database error: {0}")]
Database(String),
```

- 包装 sqlx 和 schema metadata 错误。
- 不暴露 SQLite 内部路径给 frontend protocol。
- `CoreRuntime::prepare()` 若 DB 初始化失败，直接返回 error。

## Runtime 集成

`CoreRuntime` 增加字段：

```rust
pub struct CoreRuntime {
    server: CoreIpcServer,
    worker_manager: WorkerManager,
    _database: CoreDatabase,
}
```

`_database` 当前仅持有连接池生命周期，后续 audit/grant 模块将直接使用 `pool()`。

## 测试策略

- `CoreDatabase::connect()` 创建 DB 文件。
- `schema_version()` 返回 `1`。
- 重复 connect / migration 是幂等的。
- 父目录不存在时会自动创建。
- `CoreRuntime::prepare()` 初始化 DB，不改变现有 `ping/status/dispatch` 行为。
- 现有 `kunkka-core` 测试继续通过。

## 依赖

- 在 workspace `[dependencies]` 增加 `sqlx`。
- `kunkka-core` 启用 `sqlx` SQLite + runtime tokio features。
- features: `runtime-tokio`, `sqlite`。

## 实施备注

建议实施顺序：

1. 在 workspace `Cargo.toml` 和 `kunkka-core/Cargo.toml` 添加 sqlx dependency。
2. 创建 `crates/kunkka-core/migrations/0001_core_metadata.sql`。
3. 实现 `crates/kunkka-core/src/database.rs`：`CoreDatabase`、`connect`、`schema_version`、`ping`。
4. 在 `lib.rs` 注册 `pub mod database`。
5. 集成到 `CoreRuntime::prepare()`。
6. 添加 `database` 模块 tests。
7. 添加 `CoreRuntime::prepare()` DB 初始化 test。
8. 更新 `docs/storage.md`、`docs/architecture.md`、`docs/development-log.md`。
9. 运行 workspace fmt、test、clippy 验证。
