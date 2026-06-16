# Frontend Dispatch Audit 设计

## [S1] 状态

已在 2026-06-16 确认方向；用于承接已完成的 `manifest permissions` 与 `core database foundation`。

## [S2] 背景

`kunkka-core` 已经能对 frontend dispatch 做 manifest allowlist 权限判断，也已经具备 SQLite/sqlx core database foundation，但当前这些权限决策仍是纯运行时行为，没有持久化审计记录。`docs/permissions.md` 同时把 “Core audits controlled capability access” 定义为安全原则。

下一个最小可验证切片，是把现有已实现的 frontend dispatch 权限决策写入 core 数据库，而不是一次性展开完整 permission system 或动态 grant/revoke。

## [S3] 目标

- 为 frontend dispatch 权限决策增加持久化 audit log。
- 审计落在 `kunkka-core` 自有 SQLite 数据库中。
- 仅覆盖当前已实现的三条分支：manifest 不存在、method 未授权、method 已授权。
- 不改变 IPC schema、native-host JSON、CLI 参数或 worker protocol。
- 审计写入失败时终止当前请求，避免“权限已判定但未留痕”的静默成功路径。

## [S4] 非目标

- 不实现动态 permission grant / revoke。
- 不实现 worker/file/shell/database capability 的权限审计。
- 不实现审计查询协议或 CLI 命令。
- 不记录 `invalid_request` 这类请求语法错误。
- 不改动 `kunkka-ipc`、`kunkka-protocol`、`kunkka-worker-sdk`、`kunkka-native-host`、`kunkka-cli`。

## [S5] 架构边界

保持当前最小实现边界：

- 权限判断继续由 `crates/kunkka-core/src/permissions.rs` 负责。
- 审计写入直接复用 `crates/kunkka-core/src/database.rs` 里的 `CoreDatabase`，不新增额外审计模块。
- `runtime.rs` 在 frontend dispatch handler 中负责编排“判断 -> 写审计 -> 返回/继续 dispatch”。

运行顺序：

```text
validate request
-> lookup manifest
-> decide_frontend_dispatch
-> write audit row into core DB
-> if allow: dispatch_with_start
-> if deny: return platform error
```

## [S6] 审计表

新增 migration：

```text
crates/kunkka-core/migrations/0002_frontend_dispatch_audit.sql
```

表结构：

```sql
CREATE TABLE frontend_dispatch_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_id TEXT NOT NULL,
    method TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    reason_code TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

语义：

- `decision = "allow"`：manifest 存在且 `allowed_methods` 命中。
- `decision = "deny"`：manifest 不存在或 method 未授权。
- `reason_code = "allowed" | "app_not_found" | "permission_denied"`。
- migration 使用 `CREATE TABLE` 而不是 `CREATE TABLE IF NOT EXISTS`，避免静默跳过现有同名表。

## [S7] 数据库 API

在 `crates/kunkka-core/src/database.rs` 增加最小 API：

```rust
impl CoreDatabase {
    pub async fn record_frontend_dispatch_audit(
        &self,
        app_id: &str,
        method: &str,
        decision: &str,
        reason_code: &str,
    ) -> Result<()>;
}
```

不新增专门的 audit 读取 API；测试直接通过现有 `CoreDatabase::pool()` 查询 SQLite，避免把无界全表读取接口带入生产边界。

## [S8] Runtime 行为

- `app_id` 或 `method` 为空：继续返回 `invalid_request`，不写审计；它们属于请求语法错误，不进入权限决策层。
- manifest 不存在：写入 `deny/app_not_found`，返回 `PlatformError { code: "app_not_found" }`。
- method 未授权：写入 `deny/permission_denied`，返回 `PlatformError { code: "permission_denied" }`。
- method 已授权：先写入 `allow/allowed`，再调用 `dispatch_with_start`。

## [S9] 错误处理

审计写入失败视为 core 内部错误，不静默忽略：

- 在 allow 路径里，写入失败则当前请求返回 `core_error`，且不继续 dispatch。
- 在 deny 路径里，写入失败则当前请求同样返回 `core_error`，而不是原本的 deny platform error。

理由：本切片的目标就是让权限决策可持久化；如果 audit insert 失败却继续执行业务，会让审计是否可信变得不确定。

## [S10] 测试策略

- `database` tests：验证新 migration 已创建 audit 表，`record_frontend_dispatch_audit()` 能写入记录，测试通过 SQL 直接读回并断言结果。
- `frontend_dispatch_runtime` tests：验证 `allow/allowed`、`deny/app_not_found`、`deny/permission_denied` 三条路径都会写入对应 audit row；断言同样通过 SQL 直接读取。

## [S11] 预期文件范围

- `crates/kunkka-core/migrations/0002_frontend_dispatch_audit.sql`
- `crates/kunkka-core/src/database.rs`
- `crates/kunkka-core/src/runtime.rs`
- `crates/kunkka-core/tests/database.rs`
- `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`
- `docs/permissions.md`
- `docs/storage.md`
- `docs/architecture.md`
- `docs/development-log.md`
