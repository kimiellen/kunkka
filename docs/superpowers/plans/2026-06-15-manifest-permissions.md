# Manifest Frontend Dispatch Permissions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the temporary `allow_frontend_dispatch_v1()` with manifest-based deny-by-default permission checking.

**Architecture:** Add `AppPermissions` to `AppManifest`, create a small `permissions.rs` module in `kunkka-core`, and modify `runtime.rs` to check permissions before dispatching. Existing warm-worker frontend-dispatch tests need manifest fixtures because the new permission check requires a manifest even for already-active workers.

**Tech Stack:** Rust, serde, serde_json, tempfile (dev), tokio (dev)

---

### Task 1: Add permission types to AppManifest

**Files:**
- Modify: `crates/kunkka-core/src/app_manifest.rs`
- Test: `crates/kunkka-core/tests/app_manifest.rs`

- [ ] **Step 1: Write the failing test for loading manifest with permissions**

Add to `crates/kunkka-core/tests/app_manifest.rs`:

```rust
#[test]
fn loads_manifest_with_frontend_dispatch_permissions() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
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
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert_eq!(
        manifest.permissions.frontend_dispatch.allowed_methods,
        vec!["search".to_string(), "open".to_string()]
    );
}

#[test]
fn missing_permissions_defaults_to_empty_allowed_methods() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert!(manifest.permissions.frontend_dispatch.allowed_methods.is_empty());
}

#[test]
fn missing_frontend_dispatch_defaults_to_empty_allowed_methods() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            },
            "permissions": {}
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert!(manifest.permissions.frontend_dispatch.allowed_methods.is_empty());
}

#[test]
fn rejects_blank_method_in_allowed_methods() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["search", "  "]
                }
            }
        }"#,
    );

    let err = AppRegistry::load(&paths).unwrap_err();
    assert!(matches!(
        err,
        CoreError::ManifestInvalid(message) if message.contains("allowed_methods")
    ));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kunkka-core --test app_manifest`
Expected: FAIL with "no field `permissions` on type `AppManifest`"

- [ ] **Step 3: Add permission types to app_manifest.rs**

Add to `crates/kunkka-core/src/app_manifest.rs` after `WorkerCommand`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPermissions {
    pub frontend_dispatch: FrontendDispatchPermissions,
}

impl Default for AppPermissions {
    fn default() -> Self {
        Self {
            frontend_dispatch: FrontendDispatchPermissions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendDispatchPermissions {
    pub allowed_methods: Vec<String>,
}

impl Default for FrontendDispatchPermissions {
    fn default() -> Self {
        Self {
            allowed_methods: Vec::new(),
        }
    }
}
```

Add `permissions` field to `AppManifest`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    pub permissions: AppPermissions,
    pub idle_timeout_ms: u64,
    pub startup_timeout_ms: u64,
}
```

Add raw deserialization types after `RawWorkerCommand`:

```rust
#[derive(Debug, Deserialize, Default)]
struct RawAppPermissions {
    #[serde(default)]
    frontend_dispatch: Option<RawFrontendDispatchPermissions>,
}

#[derive(Debug, Deserialize)]
struct RawFrontendDispatchPermissions {
    #[serde(default)]
    allowed_methods: Option<Vec<String>>,
}
```

Add `permissions` field to `RawAppManifest`:

```rust
#[derive(Debug, Deserialize)]
struct RawAppManifest {
    app_id: AppId,
    worker: RawWorkerCommand,
    #[serde(default)]
    permissions: Option<RawAppPermissions>,
    #[serde(default = "default_idle_timeout_ms")]
    idle_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    startup_timeout_ms: u64,
}
```

Update `AppManifest::from_raw` to convert permissions:

```rust
fn from_raw(raw: RawAppManifest, path: &Path) -> Result<Self> {
    let program = raw.worker.program.ok_or_else(|| {
        CoreError::ManifestInvalid(format!("{}: worker.program is required", path.display()))
    })?;
    let args = raw.worker.args.ok_or_else(|| {
        CoreError::ManifestInvalid(format!("{}: worker.args is required", path.display()))
    })?;

    let permissions = match raw.permissions {
        Some(raw_perms) => {
            let frontend_dispatch = match raw_perms.frontend_dispatch {
                Some(raw_fd) => {
                    let methods = raw_fd.allowed_methods.unwrap_or_default();
                    FrontendDispatchPermissions {
                        allowed_methods: methods,
                    }
                }
                None => FrontendDispatchPermissions::default(),
            };
            AppPermissions { frontend_dispatch }
        }
        None => AppPermissions::default(),
    };

    Ok(Self {
        app_id: raw.app_id,
        worker: WorkerCommand {
            program,
            args,
            env: raw.worker.env,
            cwd: raw.worker.cwd,
        },
        permissions,
        idle_timeout_ms: raw.idle_timeout_ms,
        startup_timeout_ms: raw.startup_timeout_ms,
    })
}
```

Update `AppManifest::validate` to check for blank methods in `allowed_methods`:

```rust
fn validate(&self, path: &Path) -> Result<()> {
    if self.app_id.as_str().trim().is_empty() {
        return Err(CoreError::ManifestInvalid(format!(
            "{}: app_id is required",
            path.display()
        )));
    }

    if self.worker.program.trim().is_empty() {
        return Err(CoreError::ManifestInvalid(format!(
            "{}: worker.program is required",
            path.display()
        )));
    }

    for method in &self.permissions.frontend_dispatch.allowed_methods {
        if method.trim().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: permissions.frontend_dispatch.allowed_methods contains blank method",
                path.display()
            )));
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kunkka-core --test app_manifest`
Expected: All tests PASS (including new and existing tests)

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-core/src/app_manifest.rs crates/kunkka-core/tests/app_manifest.rs
git commit -m "feat: add AppPermissions to AppManifest"
```

---

### Task 2: Add permissions module

**Files:**
- Create: `crates/kunkka-core/src/permissions.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Test: `crates/kunkka-core/tests/permissions.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/kunkka-core/tests/permissions.rs`:

```rust
use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, FrontendDispatchPermissions, WorkerCommand,
};
use kunkka_core::permissions::{decide_frontend_dispatch, PermissionDecision};
use kunkka_worker_sdk::AppId;
use std::collections::BTreeMap;

fn manifest_with_methods(methods: &[&str]) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test-app"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        permissions: AppPermissions {
            frontend_dispatch: FrontendDispatchPermissions {
                allowed_methods: methods.iter().map(|s| s.to_string()).collect(),
            },
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[test]
fn allows_method_present_in_allowed_methods() {
    let manifest = manifest_with_methods(&["search", "open"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Allow
    ));
}

#[test]
fn denies_method_not_in_allowed_methods() {
    let manifest = manifest_with_methods(&["search"]);
    let decision = decide_frontend_dispatch(&manifest, "delete");
    assert!(matches!(
        decision,
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn denies_when_allowed_methods_is_empty() {
    let manifest = manifest_with_methods(&[]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn method_matching_is_case_sensitive() {
    let manifest = manifest_with_methods(&["Search"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn method_matching_does_not_trim() {
    let manifest = manifest_with_methods(&["search"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, " search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kunkka-core --test permissions`
Expected: FAIL with "module `permissions` does not exist"

- [ ] **Step 3: Create permissions.rs**

Create `crates/kunkka-core/src/permissions.rs`:

```rust
use crate::app_manifest::AppManifest;

pub enum PermissionDecision {
    Allow,
    Deny { code: &'static str, message: String },
}

pub fn decide_frontend_dispatch(manifest: &AppManifest, method: &str) -> PermissionDecision {
    if manifest
        .permissions
        .frontend_dispatch
        .allowed_methods
        .iter()
        .any(|allowed| allowed == method)
    {
        PermissionDecision::Allow
    } else {
        PermissionDecision::Deny {
            code: "permission_denied",
            message: format!(
                "frontend dispatch method {:?} is not allowed for app {:?}",
                method,
                manifest.app_id.as_str()
            ),
        }
    }
}
```

- [ ] **Step 4: Register the module in lib.rs**

Add to `crates/kunkka-core/src/lib.rs`:

```rust
pub mod permissions;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p kunkka-core --test permissions`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/permissions.rs crates/kunkka-core/src/lib.rs crates/kunkka-core/tests/permissions.rs
git commit -m "feat: add permissions module with decide_frontend_dispatch"
```

---

### Task 3: Wire permissions into runtime handler

**Files:**
- Modify: `crates/kunkka-core/src/worker_dispatch.rs`
- Modify: `crates/kunkka-core/src/runtime.rs`
- Test: `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`

- [ ] **Step 1: Add app_registry getter to WorkerManager**

Add to `crates/kunkka-core/src/worker_dispatch.rs` after the `registry()` method (line 63):

```rust
pub fn app_registry(&self) -> &AppRegistry {
    &self.app_registry
}
```

- [ ] **Step 2: Write the failing test for permission denied**

Add to `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`:

First, add a helper function to write manifests (add after `test_paths`):

```rust
fn write_manifest(paths: &KunkkaPaths, body: &str) {
    use std::fs;
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("notes.json"), body).unwrap();
}
```

Then add the test:

```rust
#[tokio::test]
async fn frontend_dispatch_denies_method_not_in_manifest_permissions() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["open"]
                }
            }
        }"#,
    );
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frontend_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection
                .send_frame(&dispatch_frame(30, "notes", "search"))
                .await
                .unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();

    let Frame::Response { payload, .. } = wait_for(frontend_task).await.unwrap() else {
        panic!("expected response frame");
    };
    let FrontendDispatchMessage::DispatchResult(FrontendDispatchResponse::PlatformError {
        code,
        message,
    }) = decode_frontend_dispatch_message(&payload).unwrap()
    else {
        panic!("expected platform error");
    };
    assert_eq!(code, "permission_denied");
    assert!(message.contains("search"));
    assert!(message.contains("notes"));
    assert!(!runtime.worker_manager().is_active(&AppId::new("notes")));
}
```

- [ ] **Step 3: Run the new test to verify it fails**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime frontend_dispatch_denies_method_not_in_manifest_permissions`
Expected: FAIL (test passes because `allow_frontend_dispatch_v1` still returns `true`)

- [ ] **Step 4: Update runtime.rs to use permissions**

Replace the `handle_frontend_dispatch_request` function in `crates/kunkka-core/src/runtime.rs`:

```rust
async fn handle_frontend_dispatch_request(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    request: FrontendDispatchRequest,
) -> FrontendDispatchResponse {
    if request.app_id.is_empty() {
        return platform_error("invalid_request", "dispatch app_id is empty");
    }
    if request.method.is_empty() {
        return platform_error("invalid_request", "dispatch method is empty");
    }

    let Some(manifest) = worker_manager.app_registry().get(&request.app_id) else {
        return platform_error(
            "app_not_found",
            format!("app not found: {}", request.app_id),
        );
    };

    match crate::permissions::decide_frontend_dispatch(manifest, &request.method) {
        crate::permissions::PermissionDecision::Deny { code, message } => {
            return platform_error(code, message);
        }
        crate::permissions::PermissionDecision::Allow => {}
    }

    match worker_manager
        .dispatch_with_start(
            server,
            AppId::new(request.app_id),
            request.method,
            request.payload,
        )
        .await
    {
        Ok(DispatchResult::Ok(payload)) => FrontendDispatchResponse::Ok(payload),
        Ok(DispatchResult::AppError { code, message }) => {
            FrontendDispatchResponse::AppError { code, message }
        }
        Err(err) => platform_error(dispatch_platform_error_code(&err), err.to_string()),
    }
}
```

Remove the `allow_frontend_dispatch_v1` function and its TODO comment (lines 236-239 in the original file).

- [ ] **Step 5: Run the new test to verify it passes**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime frontend_dispatch_denies_method_not_in_manifest_permissions`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/worker_dispatch.rs crates/kunkka-core/src/runtime.rs crates/kunkka-core/tests/frontend_dispatch_runtime.rs
git commit -m "feat: wire manifest permissions into frontend dispatch handler"
```

---

### Task 4: Fix existing frontend dispatch tests that need manifest fixtures

**Files:**
- Modify: `crates/kunkka-core/tests/frontend_dispatch_runtime.rs`

The following existing tests dispatch to app "notes" via warm worker but have no manifest. The new permission check requires a manifest:
- `frontend_dispatch_calls_warm_worker_and_returns_payload`
- `frontend_dispatch_returns_worker_app_error`
- `one_frontend_connection_can_handle_status_then_dispatch`

- [ ] **Step 1: Run existing frontend dispatch tests to see which fail**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime`
Expected: 3 tests FAIL with `app_not_found` for "notes"

- [ ] **Step 2: Add a shared manifest helper**

Add to `crates/kunkka-core/tests/frontend_dispatch_runtime.rs` (if not already added in Task 3):

```rust
fn write_manifest(paths: &KunkkaPaths, body: &str) {
    use std::fs;
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("notes.json"), body).unwrap();
}

fn write_notes_manifest_with_search(paths: &KunkkaPaths) {
    write_manifest(
        paths,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["search"]
                }
            }
        }"#,
    );
}
```

- [ ] **Step 3: Fix `frontend_dispatch_calls_warm_worker_and_returns_payload`**

Add `write_notes_manifest_with_search(&paths);` after `let mut runtime = prepare_core_runtime(&paths).await.unwrap();` in the test.

- [ ] **Step 4: Fix `frontend_dispatch_returns_worker_app_error`**

Add `write_notes_manifest_with_search(&paths);` after `let mut runtime = prepare_core_runtime(&paths).await.unwrap();` in the test.

- [ ] **Step 5: Fix `one_frontend_connection_can_handle_status_then_dispatch`**

Add `write_notes_manifest_with_search(&paths);` after `let mut runtime = prepare_core_runtime(&paths).await.unwrap();` in the test.

- [ ] **Step 6: Run all frontend dispatch tests**

Run: `cargo test -p kunkka-core --test frontend_dispatch_runtime`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/kunkka-core/tests/frontend_dispatch_runtime.rs
git commit -m "fix: add manifest fixtures to existing frontend dispatch tests"
```

---

### Task 5: Update native-host integration test

**Files:**
- Modify: `crates/kunkka-native-host/tests/bridge_session.rs`

The `session_reuses_connection_for_status_then_dispatch` test dispatches to "example-app" which has no manifest. The new permission check will reject it.

- [ ] **Step 1: Run native-host tests to see which fail**

Run: `cargo test -p kunkka-native-host --test bridge_session`
Expected: `session_reuses_connection_for_status_then_dispatch` FAILS

- [ ] **Step 2: Add manifest fixture to the test**

Add a helper to `crates/kunkka-native-host/tests/bridge_session.rs`:

```rust
fn write_manifest(paths: &KunkkaPaths, body: &str) {
    use std::fs;
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join("example-app.json"), body).unwrap();
}
```

Add `write_manifest` call in `session_reuses_connection_for_status_then_dispatch` after `let mut runtime = prepare_core_runtime(&paths).await.unwrap();`:

```rust
write_manifest(
    &paths,
    r#"{
        "app_id": "example-app",
        "worker": {
            "program": "/usr/bin/example-worker",
            "args": []
        },
        "permissions": {
            "frontend_dispatch": {
                "allowed_methods": ["search"]
            }
        }
    }"#,
);
```

- [ ] **Step 3: Run native-host tests to verify they pass**

Run: `cargo test -p kunkka-native-host --test bridge_session`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-native-host/tests/bridge_session.rs
git commit -m "fix: add manifest fixture to native-host dispatch integration test"
```

---

### Task 6: Update documentation

**Files:**
- Modify: `docs/permissions.md`
- Modify: `docs/architecture.md`
- Modify: `docs/development-log.md`

- [ ] **Step 1: Update permissions.md**

Replace the "Current Status" and "Current Frontend Dispatch Status" sections in `docs/permissions.md`:

```markdown
## Current Status

Permission enforcement is partially implemented.

Frontend dispatch is now checked against manifest-declared `permissions.frontend_dispatch.allowed_methods`. If the method is not in the allowed list, core returns `permission_denied`. If no manifest exists for the app, core returns `app_not_found`.

Worker invocation, database access, file access, shell execution, and other controlled capabilities are not yet enforced.

## Current Frontend Dispatch Status

Frontend dispatch permission is checked in `kunkka-core` against the app manifest:

- `permissions.frontend_dispatch.allowed_methods` declares which methods are allowed.
- Missing `permissions`, missing `frontend_dispatch`, or empty `allowed_methods` means deny all.
- Permission decision is in `crates/kunkka-core/src/permissions.rs`.
- `native-host` does not make permission decisions.
```

- [ ] **Step 2: Update architecture.md**

In `docs/architecture.md`, update the "当前实现切片" section to mention manifest permissions:

Add to the `kunkka-core` bullet point:

```markdown
- `kunkka-core`：...、frontend-dispatch runtime handler、manifest-based frontend dispatch permissions，以及 idle worker cleanup。
```

- [ ] **Step 3: Update development-log.md**

Add to the top of `docs/development-log.md`:

```markdown
### Manifest Frontend Dispatch Permissions

Implemented:

- `AppPermissions` and `FrontendDispatchPermissions` types in `kunkka-core/src/app_manifest.rs`.
- `permissions.rs` module with `decide_frontend_dispatch` and `PermissionDecision`.
- Runtime handler uses manifest permissions instead of `allow_frontend_dispatch_v1()`.
- Deny-by-default: missing permissions, empty `allowed_methods`, or method not in list returns `permission_denied`.
- Manifest loading validates that `allowed_methods` does not contain blank strings.

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 4: Run workspace verification**

Run:
```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add docs/permissions.md docs/architecture.md docs/development-log.md
git commit -m "docs: update permissions, architecture, and development log"
```
