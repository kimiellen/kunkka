# Worker SDK Capability Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `kunkka-worker-sdk::call_capability` 补一条直连真实 core runtime 的集成测试，验证 worker 主动 capability 调用链路可用。

**Architecture:** 保持现有 SDK API 不变，只在 `kunkka-worker-sdk` 的测试层补真实 runtime 场景。测试使用临时 XDG 目录、写入带 `capabilities.fs.paths` 的 manifest、启动 `prepare_core_runtime()`，再通过 `call_capability` 访问文件系统能力并断言响应。

**Tech Stack:** Rust, Tokio, tempfile, postcard, kunkka-core

---

## File Structure

```text
crates/kunkka-worker-sdk/
├── Cargo.toml                         # Modify: add kunkka-core dev-dependency
└── tests/
    └── capability_runtime.rs         # Create: end-to-end runtime integration test
```

---

### Task 1: Runtime-backed Capability Test

**Covers:** [S2, S6]

**Files:**
- Modify: `crates/kunkka-worker-sdk/Cargo.toml`
- Create: `crates/kunkka-worker-sdk/tests/capability_runtime.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/kunkka-worker-sdk/tests/capability_runtime.rs`:

```rust
use kunkka_core::capability::fs::{ReadFileParams, ReadFileResult};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_worker_sdk::{call_capability, AppId};
use tempfile::{tempdir, TempDir};

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

fn write_manifest_with_fs(paths: &KunkkaPaths, allowed_dir: &std::path::Path) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    std::fs::write(
        apps_dir.join("notes.json"),
        format!(
            r#"{{
                "app_id": "notes",
                "worker": {{
                    "program": "/usr/bin/notes-worker",
                    "args": ["--serve"]
                }},
                "capabilities": {{
                    "fs": {{
                        "paths": ["{}/"]
                    }}
                }}
            }}"#,
            allowed_dir.display()
        ),
    )
    .unwrap();
}

#[tokio::test]
async fn call_capability_reads_file_from_core_runtime() {
    let (_root, paths) = test_paths();
    let workspace = tempdir().unwrap();
    let file_path = workspace.path().join("note.txt");
    std::fs::write(&file_path, "hello from runtime").unwrap();
    write_manifest_with_fs(&paths, workspace.path());

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let params = postcard::to_stdvec(&ReadFileParams {
        path: file_path.to_string_lossy().into_owned(),
    })
    .unwrap();

    let client = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            call_capability(&socket_path, &AppId::new("notes"), "fs", "read_file", params)
                .await
                .unwrap()
        }
    });

    runtime.run_once().await.unwrap();
    let response = client.await.unwrap();
    let result = response.result.unwrap();
    let read_result: ReadFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(read_result.content, "hello from runtime");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kunkka-worker-sdk --test capability_runtime`
Expected: FAIL with unresolved import for `kunkka_core`

- [ ] **Step 3: Add the minimal test dependency**

In `crates/kunkka-worker-sdk/Cargo.toml` add:

```toml
[dev-dependencies]
kunkka-core = { path = "../kunkka-core" }
tempfile.workspace = true
tokio.workspace = true
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kunkka-worker-sdk --test capability_runtime`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-worker-sdk/Cargo.toml crates/kunkka-worker-sdk/tests/capability_runtime.rs docs/superpowers/plans/2026-06-17-worker-sdk-capability-runtime.md
git commit -m "test: add runtime coverage for worker capability client"
```

---

### Task 2: Focused Verification

**Covers:** [S6]

**Files:**
- Test: `crates/kunkka-worker-sdk/tests/capability_client.rs`
- Test: `crates/kunkka-worker-sdk/tests/capability_runtime.rs`

- [ ] **Step 1: Run focused SDK capability tests**

Run: `cargo test -p kunkka-worker-sdk --test capability_client --test capability_runtime`
Expected: PASS

- [ ] **Step 2: Run workspace formatting check**

Run: `cargo fmt --all --check`
Expected: PASS

- [ ] **Step 3: Commit if verification changed files**

```bash
git add crates/kunkka-worker-sdk/Cargo.toml crates/kunkka-worker-sdk/tests/capability_runtime.rs docs/superpowers/plans/2026-06-17-worker-sdk-capability-runtime.md
git commit -m "test: verify worker capability runtime path"
```
