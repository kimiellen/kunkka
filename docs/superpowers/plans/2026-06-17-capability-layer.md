# Capability Layer (File System) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement file system capability (read_file, write_file, list_dir) in kunkka-core, with manifest-based path whitelist permissions and integration tests.

**Architecture:** Worker sends capability requests via short-lived IPC connections using `kunkka.capability.v1` schema. Core routes to capability handler, checks manifest path whitelist, executes file operations, returns results. Protocol types and codec live inside kunkka-core (not kunkka-protocol).

**Tech Stack:** Rust, kunkka-ipc, postcard, serde, tokio, tempfile

---

## File Structure

```text
crates/kunkka-core/
├── src/
│   ├── app_manifest.rs      # Modify: add CapabilitiesConfig, FsCapabilityConfig
│   ├── capability/
│   │   ├── mod.rs            # Create: protocol types, codec, request router
│   │   ├── fs.rs             # Create: file system operations
│   │   └── permissions.rs    # Create: path normalization and whitelist matching
│   ├── runtime.rs            # Modify: add CAPABILITY_SCHEMA dispatch branch
│   └── lib.rs                # Modify: add capability module
└── tests/
    └── capability_fs.rs      # Create: integration tests
```

---

### Task 1: Manifest Capabilities Field

**Covers:** [S4]

**Files:**
- Modify: `crates/kunkka-core/src/app_manifest.rs`
- Test: `crates/kunkka-core/tests/app_manifest.rs`

- [ ] **Step 1: Add capability config types**

In `crates/kunkka-core/src/app_manifest.rs`, add after `FrontendDispatchPermissions`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitiesConfig {
    pub fs: Option<FsCapabilityConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FsCapabilityConfig {
    pub paths: Vec<String>,
}
```

- [ ] **Step 2: Add raw deserialization types**

In `crates/kunkka-core/src/app_manifest.rs`, add after `RawFrontendDispatchPermissions`:

```rust
#[derive(Debug, Deserialize, Default)]
struct RawCapabilitiesConfig {
    #[serde(default)]
    fs: Option<RawFsCapabilityConfig>,
}

#[derive(Debug, Deserialize)]
struct RawFsCapabilityConfig {
    #[serde(default)]
    paths: Option<Vec<String>>,
}
```

- [ ] **Step 3: Add capabilities field to AppManifest**

In `crates/kunkka-core/src/app_manifest.rs`, add `capabilities` to `AppManifest`:

```rust
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    pub permissions: AppPermissions,
    pub capabilities: CapabilitiesConfig,
    pub idle_timeout_ms: u64,
    pub startup_timeout_ms: u64,
}
```

- [ ] **Step 4: Add capabilities to RawAppManifest**

```rust
struct RawAppManifest {
    app_id: AppId,
    worker: RawWorkerCommand,
    #[serde(default)]
    permissions: Option<RawAppPermissions>,
    #[serde(default)]
    capabilities: Option<RawCapabilitiesConfig>,
    #[serde(default = "default_idle_timeout_ms")]
    idle_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    startup_timeout_ms: u64,
}
```

- [ ] **Step 5: Update from_raw to parse capabilities**

In `AppManifest::from_raw`, after permissions parsing, add:

```rust
let capabilities = match raw.capabilities {
    Some(raw_caps) => {
        let fs = raw_caps.fs.map(|raw_fs| FsCapabilityConfig {
            paths: raw_fs.paths.unwrap_or_default(),
        });
        CapabilitiesConfig { fs }
    }
    None => CapabilitiesConfig::default(),
};
```

And include `capabilities` in the returned `AppManifest`.

- [ ] **Step 6: Add validation for capability paths**

In `AppManifest::validate`, add after the frontend_dispatch method validation:

```rust
if let Some(fs_config) = &self.capabilities.fs {
    for path in &fs_config.paths {
        if path.trim().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: capabilities.fs.paths contains blank path",
                path_display
            )));
        }
        if !path.starts_with('/') {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: capabilities.fs.paths path {:?} must be absolute",
                path_display, path
            )));
        }
    }
}
```

- [ ] **Step 7: Add manifest test for capabilities parsing**

In `crates/kunkka-core/tests/app_manifest.rs`, add:

```rust
#[test]
fn manifest_loads_capabilities_fs_paths() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(
        &path,
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []},
            "capabilities": {
                "fs": {
                    "paths": ["/home/user/notes/", "/tmp/export.txt"]
                }
            }
        }"#,
    )
    .unwrap();

    let manifest = kunkka_core::app_manifest::AppManifest::load_file(&path).unwrap();
    let fs = manifest.capabilities.fs.unwrap();
    assert_eq!(fs.paths, vec!["/home/user/notes/", "/tmp/export.txt"]);
}

#[test]
fn manifest_missing_capabilities_is_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(
        &path,
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []}
        }"#,
    )
    .unwrap();

    let manifest = kunkka_core::app_manifest::AppManifest::load_file(&path).unwrap();
    assert!(manifest.capabilities.fs.is_none());
}

#[test]
fn manifest_rejects_relative_fs_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(
        &path,
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []},
            "capabilities": {
                "fs": {
                    "paths": ["relative/path"]
                }
            }
        }"#,
    )
    .unwrap();

    let err = kunkka_core::app_manifest::AppManifest::load_file(&path).unwrap_err();
    assert!(err.to_string().contains("must be absolute"));
}
```

- [ ] **Step 8: Run tests**

Run: `cargo test -p kunkka-core --test app_manifest`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/kunkka-core/src/app_manifest.rs crates/kunkka-core/tests/app_manifest.rs
git commit -m "feat: add capabilities.fs.paths to app manifest"
```

---

### Task 2: Capability Protocol Types and Codec

**Covers:** [S2, S5]

**Files:**
- Create: `crates/kunkka-core/src/capability/mod.rs`
- Modify: `crates/kunkka-core/src/lib.rs`

- [ ] **Step 1: Create capability/mod.rs with protocol types and codec**

```rust
pub mod fs;
pub mod permissions;

use kunkka_ipc::{FrameMetadata, Payload};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_CONTENT_TYPE: &str = "application/vnd.kunkka.capability.v1+postcard";
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityError {
    pub code: String,
    pub message: String,
}

pub fn encode_capability_request(request: &CapabilityRequest) -> crate::Result<Payload> {
    let bytes = postcard::to_stdvec(request)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_request(payload: &Payload) -> crate::Result<CapabilityRequest> {
    postcard::from_bytes(&payload.bytes)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}

pub fn encode_capability_response(response: &CapabilityResponse) -> crate::Result<Payload> {
    let bytes = postcard::to_stdvec(response)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability encode: {e}")))?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_response(payload: &Payload) -> crate::Result<CapabilityResponse> {
    postcard::from_bytes(&payload.bytes)
        .map_err(|e| crate::CoreError::InvalidCoreFrame(format!("capability decode: {e}")))
}
```

- [ ] **Step 2: Create placeholder fs.rs and permissions.rs**

`crates/kunkka-core/src/capability/fs.rs`:
```rust
```

`crates/kunkka-core/src/capability/permissions.rs`:
```rust
```

- [ ] **Step 3: Add capability module to lib.rs**

In `crates/kunkka-core/src/lib.rs`, add:

```rust
pub mod capability;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p kunkka-core`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-core/src/capability/ crates/kunkka-core/src/lib.rs
git commit -m "feat: add capability protocol types and codec"
```

---

### Task 3: Path Permissions

**Covers:** [S4, S5]

**Files:**
- Modify: `crates/kunkka-core/src/capability/permissions.rs`
- Create: `crates/kunkka-core/tests/capability_permissions.rs`

- [ ] **Step 1: Write permission unit tests**

`crates/kunkka-core/tests/capability_permissions.rs`:

```rust
use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, FsCapabilityConfig, FrontendDispatchPermissions,
    WorkerCommand,
};
use kunkka_core::capability::permissions::check_fs_permission;
use kunkka_worker_sdk::{AppId, WorkerCapability};

fn manifest_with_fs_paths(paths: Vec<&str>) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig {
            fs: Some(FsCapabilityConfig {
                paths: paths.into_iter().map(String::from).collect(),
            }),
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

fn manifest_without_fs() -> AppManifest {
    AppManifest {
        app_id: AppId::new("test"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[test]
fn allows_exact_file_match() {
    let manifest = manifest_with_fs_paths(vec!["/tmp/export.txt"]);
    assert!(check_fs_permission(&manifest, "/tmp/export.txt").is_ok());
}

#[test]
fn denies_exact_file_mismatch() {
    let manifest = manifest_with_fs_paths(vec!["/tmp/export.txt"]);
    assert!(check_fs_permission(&manifest, "/tmp/other.txt").is_err());
}

#[test]
fn allows_directory_prefix_match() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/notes/todo.txt").is_ok());
    assert!(check_fs_permission(&manifest, "/home/user/notes/sub/item.md").is_ok());
}

#[test]
fn denies_directory_prefix_mismatch() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/other/file.txt").is_err());
}

#[test]
fn denies_when_no_fs_config() {
    let manifest = manifest_without_fs();
    assert!(check_fs_permission(&manifest, "/tmp/file.txt").is_err());
}

#[test]
fn normalizes_dot_segments() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/notes/../notes/todo.txt").is_ok());
}

#[test]
fn normalizes_double_slashes() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user//notes/todo.txt").is_ok());
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p kunkka-core --test capability_permissions`
Expected: FAIL (module/function not found)

- [ ] **Step 3: Implement path normalization and permission check**

`crates/kunkka-core/src/capability/permissions.rs`:

```rust
use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use std::path::{Component, PathBuf};

pub fn check_fs_permission(manifest: &AppManifest, path: &str) -> Result<(), CapabilityError> {
    let fs_config = manifest
        .capabilities
        .fs
        .as_ref()
        .ok_or_else(|| CapabilityError {
            code: "permission_denied".to_string(),
            message: "app has no fs capability configured".to_string(),
        })?;

    if fs_config.paths.is_empty() {
        return Err(CapabilityError {
            code: "permission_denied".to_string(),
            message: "app fs capability has no allowed paths".to_string(),
        });
    }

    let normalized = normalize_path(path);

    for allowed in &fs_config.paths {
        if allowed.ends_with('/') {
            let prefix = normalize_path(allowed);
            if normalized.starts_with(&prefix) || normalized == prefix.trim_end_matches('/') {
                return Ok(());
            }
        } else {
            let exact = normalize_path(allowed);
            if normalized == exact {
                return Ok(());
            }
        }
    }

    Err(CapabilityError {
        code: "permission_denied".to_string(),
        message: format!(
            "path {:?} is not in allowed fs paths for app {:?}",
            path, manifest.app_id
        ),
    })
}

fn normalize_path(path: &str) -> String {
    let path = PathBuf::from(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(c) => components.push(c.to_string_lossy().into_owned()),
            Component::RootDir => components.push(String::new()),
            Component::ParentDir => {
                if components.len() > 1 {
                    components.pop();
                }
            }
            Component::CurDir => {}
            _ => {}
        }
    }
    let result = components.join("/");
    if result.is_empty() {
        "/".to_string()
    } else if path.to_string_lossy().ends_with('/') && !result.ends_with('/') {
        format!("{result}/")
    } else {
        result
    }
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p kunkka-core --test capability_permissions`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-core/src/capability/permissions.rs crates/kunkka-core/tests/capability_permissions.rs
git commit -m "feat: add capability path permission checking"
```

---

### Task 4: File System Operations

**Covers:** [S3, S5]

**Files:**
- Modify: `crates/kunkka-core/src/capability/fs.rs`
- Create: `crates/kunkka-core/tests/capability_fs_ops.rs`

- [ ] **Step 1: Write fs operation tests**

`crates/kunkka-core/tests/capability_fs_ops.rs`:

```rust
use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, FsCapabilityConfig, FrontendDispatchPermissions,
    WorkerCommand,
};
use kunkka_core::capability::fs::{handle_fs_request, ListDirResult, ReadFileResult, WriteFileResult};
use kunkka_core::capability::CapabilityError;
use kunkka_worker_sdk::AppId;
use std::fs;

fn manifest_with_dir(dir: &std::path::Path) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig {
            fs: Some(FsCapabilityConfig {
                paths: vec![format!("{}/", dir.display())],
            }),
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[tokio::test]
async fn read_file_returns_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let manifest = manifest_with_dir(dir.path());

    let result = handle_fs_request(&manifest, "read_file", &postcard::to_stdvec(&serde_json::json!({
        "path": format!("{}/hello.txt", dir.path().display())
    })).unwrap())
    .await
    .unwrap();

    let parsed: ReadFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(parsed.content, "hello world");
}

#[tokio::test]
async fn write_file_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());
    let target = format!("{}/output.txt", dir.path().display());

    let result = handle_fs_request(&manifest, "write_file", &postcard::to_stdvec(&serde_json::json!({
        "path": &target,
        "content": "written data"
    })).unwrap())
    .await
    .unwrap();

    let parsed: WriteFileResult = postcard::from_bytes(&result).unwrap();
    assert!(parsed.bytes_written > 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), "written data");
}

#[tokio::test]
async fn list_dir_returns_entries() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::write(dir.path().join("b.txt"), "b").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    let manifest = manifest_with_dir(dir.path());

    let result = handle_fs_request(&manifest, "list_dir", &postcard::to_stdvec(&serde_json::json!({
        "path": format!("{}", dir.path().display())
    })).unwrap())
    .await
    .unwrap();

    let parsed: ListDirResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(parsed.entries.len(), 3);
    let names: Vec<&str> = parsed.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
    assert!(names.contains(&"sub"));
}

#[tokio::test]
async fn read_file_denied_outside_whitelist() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());

    let result = handle_fs_request(&manifest, "read_file", &postcard::to_stdvec(&serde_json::json!({
        "path": "/etc/passwd"
    })).unwrap())
    .await;

    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "permission_denied"));
}

#[tokio::test]
async fn unknown_method_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());

    let result = handle_fs_request(&manifest, "no_such_method", &[]).await;
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "unknown_method"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p kunkka-core --test capability_fs_ops`
Expected: FAIL (module/function not found)

- [ ] **Step 3: Implement fs operations**

`crates/kunkka-core/src/capability/fs.rs`:

```rust
use crate::app_manifest::AppManifest;
use crate::capability::permissions::check_fs_permission;
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileParams {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub bytes_written: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirParams {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirResult {
    pub entries: Vec<DirEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub entry_type: String,
    pub size: u64,
}

pub async fn handle_fs_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, CapabilityError> {
    match method {
        "read_file" => {
            let p: ReadFileParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("failed to decode read_file params: {e}"),
            })?;
            check_fs_permission(manifest, &p.path)?;
            let content = tokio::fs::read_to_string(&p.path)
                .await
                .map_err(|e| io_error(&e))?;
            let result = ReadFileResult { content };
            postcard::to_stdvec(&result).map_err(|e| encode_error(e))
        }
        "write_file" => {
            let p: WriteFileParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("failed to decode write_file params: {e}"),
            })?;
            check_fs_permission(manifest, &p.path)?;
            tokio::fs::write(&p.path, &p.content)
                .await
                .map_err(|e| io_error(&e))?;
            let result = WriteFileResult {
                bytes_written: p.content.len() as u64,
            };
            postcard::to_stdvec(&result).map_err(|e| encode_error(e))
        }
        "list_dir" => {
            let p: ListDirParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
                code: "invalid_params".to_string(),
                message: format!("failed to decode list_dir params: {e}"),
            })?;
            check_fs_permission(manifest, &p.path)?;
            let mut entries = Vec::new();
            let mut read_dir = tokio::fs::read_dir(&p.path)
                .await
                .map_err(|e| io_error(&e))?;
            while let Some(entry) = read_dir.next_entry().await.map_err(|e| io_error(&e))? {
                let file_type = entry.file_type().await.map_err(|e| io_error(&e))?;
                let metadata = entry.metadata().await.map_err(|e| io_error(&e))?;
                entries.push(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    entry_type: if file_type.is_dir() {
                        "dir".to_string()
                    } else if file_type.is_symlink() {
                        "symlink".to_string()
                    } else {
                        "file".to_string()
                    },
                    size: metadata.len(),
                });
            }
            let result = ListDirResult { entries };
            postcard::to_stdvec(&result).map_err(|e| encode_error(e))
        }
        _ => Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown fs method: {method:?}"),
        }),
    }
}

fn io_error(e: &std::io::Error) -> CapabilityError {
    CapabilityError {
        code: "io_error".to_string(),
        message: e.to_string(),
    }
}

fn encode_error(e: postcard::Error) -> CapabilityError {
    CapabilityError {
        code: "encode_error".to_string(),
        message: e.to_string(),
    }
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p kunkka-core --test capability_fs_ops`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-core/src/capability/fs.rs crates/kunkka-core/tests/capability_fs_ops.rs
git commit -m "feat: add file system capability operations"
```

---

### Task 5: Runtime Capability Dispatch

**Covers:** [S5]

**Files:**
- Modify: `crates/kunkka-core/src/runtime.rs`

- [ ] **Step 1: Add capability import to runtime.rs**

Add to imports in `crates/kunkka-core/src/runtime.rs`:

```rust
use crate::capability::{
    decode_capability_request, encode_capability_response, handle_capability_request,
    CAPABILITY_SCHEMA,
};
```

- [ ] **Step 2: Add CAPABILITY_SCHEMA to run_connection dispatch**

In `run_connection()`, add the capability schema branch:

```rust
match frame_schema(&first_frame) {
    Some(WORKER_PROTOCOL_SCHEMA) => {
        worker_manager
            .handle_registration_connection(first_frame, connection, None, 300_000)
            .await
    }
    Some(CORE_CONTROL_SCHEMA | FRONTEND_DISPATCH_SCHEMA) => {
        run_frontend_connection(server, worker_manager, database, connection, first_frame).await
    }
    Some(CAPABILITY_SCHEMA) => {
        handle_capability_connection(worker_manager, connection, first_frame).await
    }
    Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
        "unknown payload schema: {schema}"
    ))),
    None => Err(CoreError::InvalidCoreFrame(
        "missing payload schema".to_string(),
    )),
}
```

- [ ] **Step 3: Implement handle_capability_connection**

Add after `run_frontend_connection`:

```rust
async fn handle_capability_connection(
    worker_manager: &WorkerManager,
    mut connection: IpcConnection,
    frame: Frame,
) -> Result<()> {
    let Frame::Request {
        request_id,
        session_id,
        source,
        target,
        payload,
        ..
    } = frame
    else {
        return Err(CoreError::InvalidCoreFrame(
            "expected request frame".to_string(),
        ));
    };

    let request = decode_capability_request(&payload)?;
    let response = handle_capability_request(worker_manager, request).await;
    let response_payload = encode_capability_response(&response)?;

    let response_frame = Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload: response_payload,
        metadata: FrameMetadata::new(),
    };

    connection.send_frame(&response_frame).await?;
    Ok(())
}
```

- [ ] **Step 4: Add handle_capability_request to capability/mod.rs**

In `crates/kunkka-core/src/capability/mod.rs`, add:

```rust
use crate::worker_dispatch::WorkerManager;

pub async fn handle_capability_request(
    worker_manager: &WorkerManager,
    request: CapabilityRequest,
) -> CapabilityResponse {
    let result = handle_capability_inner(worker_manager, &request).await;
    CapabilityResponse { result }
}

async fn handle_capability_inner(
    worker_manager: &WorkerManager,
    request: &CapabilityRequest,
) -> Result<Vec<u8>, CapabilityError> {
    if request.app_id.is_empty() {
        return Err(CapabilityError {
            code: "invalid_request".to_string(),
            message: "capability request app_id is empty".to_string(),
        });
    }

    let manifest = worker_manager
        .app_registry()
        .get(&request.app_id)
        .ok_or_else(|| CapabilityError {
            code: "app_not_found".to_string(),
            message: format!("app not found: {}", request.app_id),
        })?;

    match request.capability.as_str() {
        "fs" => fs::handle_fs_request(manifest, &request.method, &request.params).await,
        _ => Err(CapabilityError {
            code: "unknown_capability".to_string(),
            message: format!("unknown capability: {}", request.capability),
        }),
    }
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p kunkka-core`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/runtime.rs crates/kunkka-core/src/capability/mod.rs
git commit -m "feat: add capability dispatch to core runtime"
```

---

### Task 6: Integration Tests

**Covers:** [S6]

**Files:**
- Create: `crates/kunkka-core/tests/capability_runtime.rs`

- [ ] **Step 1: Write integration tests**

`crates/kunkka-core/tests/capability_runtime.rs`:

```rust
use kunkka_core::capability::{
    decode_capability_response, encode_capability_request, CapabilityError, CapabilityRequest,
    CapabilityResponse, CAPABILITY_SCHEMA,
};
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::{prepare_core_runtime, CoreError};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use std::fs;
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl std::future::Future<Output = T>) -> T {
    timeout(TEST_TIMEOUT, future)
        .await
        .expect("test operation timed out")
}

fn test_paths() -> (TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    (root, paths)
}

fn write_manifest_with_fs(paths: &KunkkaPaths, allowed_paths: &[&str]) {
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    let paths_json: Vec<String> = allowed_paths.iter().map(|s| s.to_string()).collect();
    let manifest = serde_json::json!({
        "app_id": "notes",
        "worker": {
            "program": "/usr/bin/notes-worker",
            "args": ["--serve"]
        },
        "capabilities": {
            "fs": {
                "paths": paths_json
            }
        }
    });
    fs::write(
        apps_dir.join("notes.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

fn capability_frame(request_id: u128, request: &CapabilityRequest) -> Frame {
    let payload = encode_capability_request(request).unwrap();
    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("worker:notes"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

#[tokio::test]
async fn capability_read_file_returns_content() {
    let (_root, paths) = test_paths();
    let data_dir = tempdir().unwrap();
    fs::write(data_dir.path().join("hello.txt"), "hello kunkka").unwrap();
    write_manifest_with_fs(&paths, &[&format!("{}/", data_dir.path().display())]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();
    let target_path = format!("{}/hello.txt", data_dir.path().display());

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            1,
            &CapabilityRequest {
                app_id: "notes".to_string(),
                capability: "fs".to_string(),
                method: "read_file".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({"path": target_path}))
                    .unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    let content_bytes = response.result.unwrap();
    let content: String = postcard::from_bytes(&content_bytes).unwrap();
    assert_eq!(content, "hello kunkka");
}

#[tokio::test]
async fn capability_write_file_creates_file() {
    let (_root, paths) = test_paths();
    let data_dir = tempdir().unwrap();
    write_manifest_with_fs(&paths, &[&format!("{}/", data_dir.path().display())]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();
    let target_path = format!("{}/output.txt", data_dir.path().display());
    let target_path_clone = target_path.clone();

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            2,
            &CapabilityRequest {
                app_id: "notes".to_string(),
                capability: "fs".to_string(),
                method: "write_file".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({
                    "path": &target_path_clone,
                    "content": "written by capability"
                }))
                .unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    assert!(response.result.is_ok());
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "written by capability");
}

#[tokio::test]
async fn capability_list_dir_returns_entries() {
    let (_root, paths) = test_paths();
    let data_dir = tempdir().unwrap();
    fs::write(data_dir.path().join("a.txt"), "a").unwrap();
    fs::write(data_dir.path().join("b.txt"), "b").unwrap();
    write_manifest_with_fs(&paths, &[&format!("{}/", data_dir.path().display())]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();
    let dir_path = format!("{}", data_dir.path().display());

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            3,
            &CapabilityRequest {
                app_id: "notes".to_string(),
                capability: "fs".to_string(),
                method: "list_dir".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({"path": dir_path})).unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    let entries_bytes = response.result.unwrap();
    let entries: kunkka_core::capability::fs::ListDirResult =
        postcard::from_bytes(&entries_bytes).unwrap();
    assert_eq!(entries.entries.len(), 2);
}

#[tokio::test]
async fn capability_denies_path_not_in_whitelist() {
    let (_root, paths) = test_paths();
    let data_dir = tempdir().unwrap();
    write_manifest_with_fs(&paths, &[&format!("{}/", data_dir.path().display())]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            4,
            &CapabilityRequest {
                app_id: "notes".to_string(),
                capability: "fs".to_string(),
                method: "read_file".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({"path": "/etc/passwd"})).unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    let err = response.result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}

#[tokio::test]
async fn capability_denies_when_no_capabilities_config() {
    let (_root, paths) = test_paths();
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(
        apps_dir.join("notes.json"),
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []}
        }"#,
    )
    .unwrap();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            5,
            &CapabilityRequest {
                app_id: "notes".to_string(),
                capability: "fs".to_string(),
                method: "read_file".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({"path": "/tmp/x"})).unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    let err = response.result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}

#[tokio::test]
async fn capability_returns_app_not_found_for_unknown_app() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let socket_path = paths.socket_path.clone();

    let task = tokio::spawn(async move {
        let mut conn = IpcConnection::connect(&socket_path).await.unwrap();
        let frame = capability_frame(
            6,
            &CapabilityRequest {
                app_id: "no_such_app".to_string(),
                capability: "fs".to_string(),
                method: "read_file".to_string(),
                params: postcard::to_stdvec(&serde_json::json!({"path": "/tmp/x"})).unwrap(),
            },
        );
        conn.send_frame(&frame).await.unwrap();
        conn.recv_frame().await.unwrap().unwrap()
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(task).await.unwrap();

    let Frame::Response { payload, .. } = response_frame else {
        panic!("expected response frame");
    };
    let response = decode_capability_response(&payload).unwrap();
    let err = response.result.unwrap_err();
    assert_eq!(err.code, "app_not_found");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p kunkka-core --test capability_runtime`
Expected: PASS

- [ ] **Step 3: Run full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-core/tests/capability_runtime.rs
git commit -m "feat: add capability integration tests"
```
