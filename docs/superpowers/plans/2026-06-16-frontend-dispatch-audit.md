# Frontend Dispatch Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist frontend dispatch permission decisions into the core SQLite database.

**Architecture:** Add one SQLite migration and one small `CoreDatabase` audit write helper, then call it from the frontend dispatch runtime handler before returning deny results or dispatching allow results. Reuse the existing temporary-XDG integration-test style and read audit rows directly through `pool()` in tests so no production read API is added.

**Tech Stack:** Rust, sqlx/sqlite, tokio, tempfile

---

## File Map

- `crates/kunkka-core/migrations/0002_frontend_dispatch_audit.sql`: creates the audit table.
- `crates/kunkka-core/src/database.rs`: owns the audit write helper, validation, and SQL error mapping.
- `crates/kunkka-core/src/runtime.rs`: writes audit rows in the frontend dispatch permission paths.
- `crates/kunkka-core/tests/database.rs`: data-layer roundtrip coverage for the audit table.
- `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`: integration assertions that allow/deny requests persist audit rows.
- `docs/permissions.md`, `docs/storage.md`, `docs/architecture.md`, `docs/development-log.md`: document the new persisted audit behavior.

## Current Baseline

当前工作区已经有一批未提交的 in-progress 变更：migration、`database.rs` helper、`runtime.rs` 接线、focused tests、以及对应文档都已出现。执行本计划时，不要按“全新从零实现”重复添加同名代码；应先验证现状，再把现有改动收敛到这份 plan 和 spec 的最终形态。

### Task 1: Reconcile audit migration and database helpers

**Covers:** [S2, S6, S7, S10]

**Files:**
- Create: `crates/kunkka-core/migrations/0002_frontend_dispatch_audit.sql`
- Modify: `crates/kunkka-core/src/database.rs`
- Modify: `crates/kunkka-core/tests/database.rs`

- [ ] **Step 1: Run the focused database test to verify the current baseline**

Run: `cargo test -p kunkka-core --test database`
Expected: PASS, including `frontend_dispatch_audit_rows_roundtrip`. If it fails, stop and reconcile the worktree against the target snippets in the next steps.

- [ ] **Step 2: Reconcile the audit table migration to the target schema**

Ensure `crates/kunkka-core/migrations/0002_frontend_dispatch_audit.sql` is exactly:

```sql
CREATE TABLE frontend_dispatch_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_id TEXT NOT NULL,
    method TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    reason_code TEXT NOT NULL CHECK (reason_code IN ('allowed', 'app_not_found', 'permission_denied')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

- [ ] **Step 3: Reconcile the database audit helpers**

Ensure `crates/kunkka-core/src/database.rs` exposes only this write helper inside `impl CoreDatabase` before `pool()`:

```rust
pub async fn record_frontend_dispatch_audit(
    &self,
    app_id: &str,
    method: &str,
    decision: &str,
    reason_code: &str,
) -> Result<()> {
    if decision != "allow" && decision != "deny" {
        return Err(CoreError::Database(format!(
            "invalid decision: {decision}"
        )));
    }

    if reason_code != "allowed"
        && reason_code != "app_not_found"
        && reason_code != "permission_denied"
    {
        return Err(CoreError::Database(format!(
            "invalid reason_code: {reason_code}"
        )));
    }

    sqlx::query(
        "INSERT INTO frontend_dispatch_audit (app_id, method, decision, reason_code) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(app_id)
    .bind(method)
    .bind(decision)
    .bind(reason_code)
    .execute(&self.pool)
    .await
    .map_err(|err| CoreError::Database(format!(
        "failed to insert frontend dispatch audit: {err}"
    )))?;

    Ok(())
}
```

Do not add a dedicated read helper. Keep readback checks on direct SQL queries through `db.pool()`.

- [ ] **Step 4: Re-run the focused database test to verify it still passes**

Run: `cargo test -p kunkka-core --test database`
Expected: PASS, including `frontend_dispatch_audit_rows_roundtrip`.

- [ ] **Step 5: Do not commit unless the user explicitly asks**

If the user later asks for a commit, use:

```text
feat: add frontend dispatch audit database helpers
```

### Task 2: Reconcile runtime audit integration

**Covers:** [S3, S5, S8, S9, S10]

**Files:**
- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`

- [ ] **Step 1: Run the focused runtime test target to verify the current baseline**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime`
Expected: PASS, including the persisted-audit assertions. If it fails, use the next two steps as the target state.

- [ ] **Step 2: Ensure the runtime test asserts the final persisted audit rows**

Keep this helper near `dispatch_frame()` in `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`:

```rust
async fn audit_rows(runtime: &kunkka_core::runtime::CoreRuntime) -> Vec<(String, String, String, String)> {
    sqlx::query(
        "SELECT app_id, method, decision, reason_code FROM frontend_dispatch_audit ORDER BY id ASC",
    )
        .fetch_all(runtime.database().pool())
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.try_get("app_id").unwrap(),
                row.try_get("method").unwrap(),
                row.try_get("decision").unwrap(),
                row.try_get("reason_code").unwrap(),
            )
        })
        .collect()
}
```

Keep these assertions:

In `frontend_dispatch_calls_warm_worker_and_returns_payload()` after the existing worker assertion:

```rust
assert_eq!(
    audit_rows(&runtime).await,
    vec![(
        "notes".to_string(),
        "search".to_string(),
        "allow".to_string(),
        "allowed".to_string(),
    )]
);
```

In `frontend_dispatch_maps_missing_manifest_to_platform_error()` after the existing `app_not_found` assertions:

```rust
assert_eq!(
    audit_rows(&runtime).await,
    vec![(
        "missing".to_string(),
        "search".to_string(),
        "deny".to_string(),
        "app_not_found".to_string(),
    )]
);
```

In `frontend_dispatch_denies_method_not_in_manifest_permissions()` after the existing runtime activity assertion:

```rust
assert_eq!(
    audit_rows(&runtime).await,
    vec![(
        "notes".to_string(),
        "search".to_string(),
        "deny".to_string(),
        "permission_denied".to_string(),
    )]
);
```

- [ ] **Step 3: Ensure runtime threads the database through the frontend dispatch handler and records audit rows**

Ensure `crates/kunkka-core/src/runtime.rs` exposes this accessor inside `impl CoreRuntime`:

```rust
pub fn database(&self) -> &CoreDatabase {
    &self._database
}
```

Ensure `&CoreDatabase` is threaded through `run_connection()`, `run_frontend_connection()`, `handle_frontend_frame()`, and `handle_frontend_dispatch_frame()`.

Ensure `handle_frontend_dispatch_request()` returns `Result<FrontendDispatchResponse>` and uses this body:

```rust
async fn handle_frontend_dispatch_request(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    database: &CoreDatabase,
    request: FrontendDispatchRequest,
) -> Result<FrontendDispatchResponse> {
    if request.app_id.is_empty() {
        return Ok(platform_error("invalid_request", "dispatch app_id is empty"));
    }
    if request.method.is_empty() {
        return Ok(platform_error("invalid_request", "dispatch method is empty"));
    }

    let Some(manifest) = worker_manager.app_registry().get(&request.app_id) else {
        database
            .record_frontend_dispatch_audit(&request.app_id, &request.method, "deny", "app_not_found")
            .await?;
        return Ok(platform_error(
            "app_not_found",
            format!("app not found: {}", request.app_id),
        ));
    };

    match crate::permissions::decide_frontend_dispatch(manifest, &request.method) {
        crate::permissions::PermissionDecision::Deny { code, message } => {
            database
                .record_frontend_dispatch_audit(&request.app_id, &request.method, "deny", code)
                .await?;
            return Ok(platform_error(code, message));
        }
        crate::permissions::PermissionDecision::Allow => {}
    }

    database
        .record_frontend_dispatch_audit(&request.app_id, &request.method, "allow", "allowed")
        .await?;

    match worker_manager
        .dispatch_with_start(
            server,
            AppId::new(request.app_id),
            request.method,
            request.payload,
        )
        .await
    {
        Ok(DispatchResult::Ok(payload)) => Ok(FrontendDispatchResponse::Ok(payload)),
        Ok(DispatchResult::AppError { code, message }) => {
            Ok(FrontendDispatchResponse::AppError { code, message })
        }
        Err(err) => Ok(platform_error(dispatch_platform_error_code(&err), err.to_string())),
    }
}
```

- [ ] **Step 4: Re-run the focused runtime tests to verify they still pass**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime`
Expected: PASS, including the new persisted-audit assertions.

- [ ] **Step 5: Re-run the focused data + runtime tests together**

Run: `cargo test -p kunkka-core --test database --test frontend_dispatch_runtime`
Expected: PASS.

- [ ] **Step 6: Do not commit unless the user explicitly asks**

If the user later asks for a commit, use:

```text
feat: audit frontend dispatch permission decisions
```

### Task 3: Update docs and run full verification

**Covers:** [S3, S4, S10, S11]

**Files:**
- Modify: `docs/permissions.md`
- Modify: `docs/storage.md`
- Modify: `docs/architecture.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update the repository docs to match the shipped behavior**

Apply these exact doc edits:

In `docs/permissions.md`, extend the current-status paragraph and bullet list to state that both allow and deny frontend dispatch decisions are audited into the core SQLite database and that the table name is `frontend_dispatch_audit`.

In `docs/storage.md`, add one line under the core database implementation bullets:

```text
- Second migration creates `frontend_dispatch_audit` for persisted permission decision audit rows.
```

In `docs/architecture.md`, extend the `kunkka-core` implementation slice bullet to include `frontend dispatch permission audit persistence`.

In `docs/development-log.md`, add a new `### Frontend Dispatch Audit` entry with the migration, helpers, runtime behavior, and verification commands.

- [ ] **Step 2: Run formatting, workspace tests, and Clippy in repository order**

Run these commands in order:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all three commands PASS.

- [ ] **Step 3: Do not commit unless the user explicitly asks**

If the user later asks for a commit, use:

```text
docs: record frontend dispatch audit persistence
```
