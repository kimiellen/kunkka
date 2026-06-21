# HTTP Capability (External API Request) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement HTTP capability that allows workers to make HTTP requests to external APIs through core, with domain-level whitelist.

**Architecture:** Reuse `kunkka.capability.v1` schema with `capability = "http"`. Add `reqwest` as HTTP client. Domain whitelist in manifest, exact match (case-insensitive).

**Tech Stack:** Rust, reqwest, postcard, serde

---

### Task 1: Add reqwest dependency

**Covers:** S6

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/kunkka-core/Cargo.toml`

- [ ] **Step 1: Add reqwest to workspace dependencies**

Edit `Cargo.toml` (workspace root), add to `[workspace.dependencies]`:
```toml
reqwest = { version = "0.12", features = ["gzip", "deflate"] }
```

- [ ] **Step 2: Add reqwest to kunkka-core dependencies**

Edit `crates/kunkka-core/Cargo.toml`, add to `[dependencies]`:
```toml
reqwest.workspace = true
```

- [ ] **Step 3: Verify build**

Run: `cargo check -p kunkka-core`
Expected: PASS (no errors)

---

### Task 2: Add HttpCapabilityConfig to manifest

**Covers:** S3

**Files:**
- Modify: `crates/kunkka-core/src/app_manifest.rs`

- [ ] **Step 1: Add HttpCapabilityConfig struct**

In `crates/kunkka-core/src/app_manifest.rs`, add after `ShellCapabilityConfig`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HttpCapabilityConfig {
    pub domains: Vec<String>,
}
```

- [ ] **Step 2: Add http field to CapabilitiesConfig**

Modify `CapabilitiesConfig` struct:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitiesConfig {
    pub fs: Option<FsCapabilityConfig>,
    pub shell: Option<ShellCapabilityConfig>,
    pub http: Option<HttpCapabilityConfig>,
}
```

- [ ] **Step 3: Add RawHttpCapabilityConfig and parsing**

Add after `RawShellCapabilityConfig`:
```rust
#[derive(Debug, Deserialize)]
struct RawHttpCapabilityConfig {
    #[serde(default)]
    domains: Option<Vec<String>>,
}
```

Add `http` field to `RawCapabilitiesConfig`:
```rust
#[derive(Debug, Deserialize, Default)]
struct RawCapabilitiesConfig {
    #[serde(default)]
    fs: Option<RawFsCapabilityConfig>,
    #[serde(default)]
    shell: Option<RawShellCapabilityConfig>,
    #[serde(default)]
    http: Option<RawHttpCapabilityConfig>,
}
```

- [ ] **Step 4: Add parsing logic in from_raw**

In the `from_raw` method, add http parsing after shell:
```rust
let http = raw_caps.http.map(|raw_http| HttpCapabilityConfig {
    domains: raw_http.domains.unwrap_or_default(),
});
CapabilitiesConfig { fs, shell, http }
```

- [ ] **Step 5: Add validation for http domains**

In the `validate` method, add:
```rust
if let Some(http_caps) = &self.capabilities.http {
    for domain in &http_caps.domains {
        if domain.trim().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: capabilities.http.domains contains blank domain",
                path.display()
            )));
        }
    }
}
```

- [ ] **Step 6: Add manifest test**

Create `crates/kunkka-core/tests/http_manifest.rs`:
```rust
use kunkka_core::app_manifest::AppManifest;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_http_capability_config_loaded() {
    let dir = tempdir().unwrap();
    let apps_dir = dir.path().join("config/apps");
    fs::create_dir_all(&apps_dir).unwrap();

    fs::write(
        apps_dir.join("test.json"),
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            },
            "capabilities": {
                "http": {
                    "domains": ["api.github.com", "hooks.slack.com"]
                }
            }
        }"#,
    )
    .unwrap();

    let manifest = AppManifest::load_file(apps_dir.join("test.json")).unwrap();
    let http = manifest.capabilities.http.unwrap();
    assert_eq!(http.domains, vec!["api.github.com", "hooks.slack.com"]);
}

#[test]
fn test_http_capability_config_empty() {
    let dir = tempdir().unwrap();
    let apps_dir = dir.path().join("config/apps");
    fs::create_dir_all(&apps_dir).unwrap();

    fs::write(
        apps_dir.join("test.json"),
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            }
        }"#,
    )
    .unwrap();

    let manifest = AppManifest::load_file(apps_dir.join("test.json")).unwrap();
    assert!(manifest.capabilities.http.is_none());
}

#[test]
fn test_http_blank_domain_rejected() {
    let dir = tempdir().unwrap();
    let apps_dir = dir.path().join("config/apps");
    fs::create_dir_all(&apps_dir).unwrap();

    fs::write(
        apps_dir.join("test.json"),
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            },
            "capabilities": {
                "http": {
                    "domains": [""]
                }
            }
        }"#,
    )
    .unwrap();

    let result = AppManifest::load_file(apps_dir.join("test.json"));
    assert!(result.is_err());
}
```

- [ ] **Step 7: Run manifest tests**

Run: `cargo test -p kunkka-core --test http_manifest`
Expected: PASS

- [ ] **Step 8: Run all existing tests**

Run: `cargo test --workspace`
Expected: PASS (all existing tests still pass)

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/kunkka-core/Cargo.toml crates/kunkka-core/src/app_manifest.rs crates/kunkka-core/tests/http_manifest.rs
git commit -m "feat: add HttpCapabilityConfig to app manifest"
```

---

### Task 3: Implement HTTP capability handler

**Covers:** S2, S4, S5

**Files:**
- Create: `crates/kunkka-core/src/capability/http.rs`
- Modify: `crates/kunkka-core/src/capability/mod.rs`

- [ ] **Step 1: Create http.rs with types**

Create `crates/kunkka-core/src/capability/http.rs`:
```rust
use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestParams {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

const ALLOWED_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

pub async fn handle_http_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, CapabilityError> {
    if method != "request" {
        return Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown http method: {method}"),
        });
    }

    let params: HttpRequestParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid params: {e}"),
    })?;

    let url = reqwest::Url::parse(&params.url).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid url: {e}"),
    })?;

    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(CapabilityError {
            code: "scheme_not_allowed".to_string(),
            message: format!("url scheme must be http or https, got: {}", url.scheme()),
        });
    }

    let method_upper = params.method.to_uppercase();
    if !ALLOWED_METHODS.contains(&method_upper.as_str()) {
        return Err(CapabilityError {
            code: "invalid_params".to_string(),
            message: format!(
                "http method must be one of {:?}, got: {}",
                ALLOWED_METHODS, params.method
            ),
        });
    }

    let http_config = manifest.capabilities.http.as_ref().ok_or_else(|| CapabilityError {
        code: "permission_denied".to_string(),
        message: "app does not have http capability configured".to_string(),
    })?;

    let host = url.host_str().ok_or_else(|| CapabilityError {
        code: "invalid_params".to_string(),
        message: "url has no host".to_string(),
    })?;

    if !http_config
        .domains
        .iter()
        .any(|d| d.eq_ignore_ascii_case(host))
    {
        return Err(CapabilityError {
            code: "permission_denied".to_string(),
            message: format!("domain {host} is not in the allowed list"),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(|e| CapabilityError {
            code: "io_error".to_string(),
            message: format!("failed to create http client: {e}"),
        })?;

    let reqwest_method = reqwest::Method::from_bytes(method_upper.as_bytes()).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid method: {e}"),
    })?;

    let mut request = client.request(reqwest_method, url);
    for (key, value) in &params.headers {
        request = request.header(key.as_str(), value.as_str());
    }
    if let Some(body) = params.body {
        request = request.body(body);
    }

    let response = request.send().await.map_err(|e| {
        if e.is_timeout() {
            CapabilityError {
                code: "timeout".to_string(),
                message: "request timed out".to_string(),
            }
        } else {
            CapabilityError {
                code: "io_error".to_string(),
                message: format!("http request failed: {e}"),
            }
        }
    })?;

    let status_code = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response.bytes().await.map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("failed to read response body: {e}"),
    })?;

    let http_response = HttpResponse {
        status_code,
        headers,
        body: body.to_vec(),
    };

    postcard::to_stdvec(&http_response).map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("encode result: {e}"),
    })
}
```

- [ ] **Step 2: Add http module to mod.rs**

In `crates/kunkka-core/src/capability/mod.rs`, add:
```rust
pub mod http;
```

- [ ] **Step 3: Add http routing in handle_capability_inner**

In `crates/kunkka-core/src/capability/mod.rs`, add "http" case to the match in `handle_capability_inner`:
```rust
"http" => http::handle_http_request(manifest, &request.method, &request.params).await,
```

- [ ] **Step 4: Verify build**

Run: `cargo check -p kunkka-core`
Expected: PASS

- [ ] **Step 5: Run all existing tests**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/capability/http.rs crates/kunkka-core/src/capability/mod.rs
git commit -m "feat: add http capability handler with domain whitelist"
```

---

### Task 4: Add HTTP capability unit tests

**Covers:** S2, S4

**Files:**
- Create: `crates/kunkka-core/tests/http_capability.rs`

- [ ] **Step 1: Create test file**

Create `crates/kunkka-core/tests/http_capability.rs`:
```rust
use kunkka_core::app_manifest::{AppManifest, CapabilitiesConfig, HttpCapabilityConfig};
use kunkka_core::capability::http::{
    handle_http_request, HttpRequestParams, HttpResponse,
};
use kunkka_core::capability::CapabilityError;

fn make_manifest(domains: &[&str]) -> AppManifest {
    AppManifest {
        app_id: kunkka_worker_sdk::AppId::new("test"),
        worker: kunkka_core::app_manifest::WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: Default::default(),
        capabilities: CapabilitiesConfig {
            fs: None,
            shell: None,
            http: Some(HttpCapabilityConfig {
                domains: domains.iter().map(|s| s.to_string()).collect(),
            }),
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

fn encode_params(params: &HttpRequestParams) -> Vec<u8> {
    postcard::to_stdvec(params).unwrap()
}

#[tokio::test]
async fn test_unknown_method() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "unknown", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "unknown_method");
}

#[tokio::test]
async fn test_invalid_url() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "not a url".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[tokio::test]
async fn test_scheme_not_allowed() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "ftp://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "scheme_not_allowed");
}

#[tokio::test]
async fn test_domain_not_in_whitelist() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://evil.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}

#[tokio::test]
async fn test_domain_case_insensitive() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://API.GITHUB.COM".to_string(),
        headers: vec![],
        body: None,
    });
    // This will fail with io_error because the server doesn't exist,
    // but it should NOT fail with permission_denied
    let result = handle_http_request(&manifest, "request", &params).await;
    if let Err(err) = result {
        assert_ne!(err.code, "permission_denied");
    }
}

#[tokio::test]
async fn test_invalid_http_method() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "TRACE".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[tokio::test]
async fn test_no_http_capability_config() {
    let manifest = AppManifest {
        app_id: kunkka_worker_sdk::AppId::new("test"),
        worker: kunkka_core::app_manifest::WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: Default::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    };
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}
```

- [ ] **Step 2: Run unit tests**

Run: `cargo test -p kunkka-core --test http_capability`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-core/tests/http_capability.rs
git commit -m "test: add http capability unit tests"
```

---

### Task 5: Add HTTP capability runtime integration test

**Covers:** S2, S6

**Files:**
- Create: `crates/kunkka-core/tests/http_runtime.rs`

- [ ] **Step 1: Create runtime integration test**

Create `crates/kunkka-core/tests/http_runtime.rs`:
```rust
use kunkka_core::capability::http::{HttpRequestParams, HttpResponse};
use kunkka_core::capability::{
    decode_capability_response, encode_capability_request, CapabilityError, CapabilityRequest,
};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use std::future::Future;
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl Future<Output = T>) -> T {
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

fn write_manifest_with_http(paths: &KunkkaPaths, domains: &[&str]) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();

    let domains_json = domains
        .iter()
        .map(|d| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(", ");

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
                    "http": {{
                        "domains": [{domains_json}]
                    }}
                }}
            }}"#,
        ),
    )
    .unwrap();
}

fn capability_frame(request_id: u128, params: &HttpRequestParams) -> Frame {
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: "notes".to_string(),
        capability: "http".to_string(),
        method: "request".to_string(),
        params: postcard::to_stdvec(params).unwrap(),
    })
    .unwrap();

    Frame {
        session_id: SessionId(1),
        request_id: RequestId(request_id),
        endpoint_id: EndpointId(1),
        payload,
    }
}

async fn connect_and_send(paths: &KunkkaPaths, frame: Frame) -> Result<HttpResponse, CapabilityError> {
    let mut connection = IpcConnection::connect(&paths.socket_path).await.unwrap();
    connection.send_frame(&frame).await.unwrap();
    let response_frame = connection.recv_frame().await.unwrap();
    let response = decode_capability_response(&response_frame.payload).unwrap();
    response.result.map(|bytes| postcard::from_bytes(&bytes).unwrap())
}

#[tokio::test]
async fn test_http_domain_not_allowed() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);

    let runtime_paths = paths.clone();
    tokio::spawn(async move {
        prepare_core_runtime(&runtime_paths).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let params = HttpRequestParams {
        method: "GET".to_string(),
        url: "https://evil.com/test".to_string(),
        headers: vec![],
        body: None,
    };
    let frame = capability_frame(1, &params);
    let result = connect_and_send(&paths, frame).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}

#[tokio::test]
async fn test_http_invalid_url() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);

    let runtime_paths = paths.clone();
    tokio::spawn(async move {
        prepare_core_runtime(&runtime_paths).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let params = HttpRequestParams {
        method: "GET".to_string(),
        url: "not a url".to_string(),
        headers: vec![],
        body: None,
    };
    let frame = capability_frame(1, &params);
    let result = connect_and_send(&paths, frame).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[tokio::test]
async fn test_http_scheme_not_allowed() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);

    let runtime_paths = paths.clone();
    tokio::spawn(async move {
        prepare_core_runtime(&runtime_paths).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let params = HttpRequestParams {
        method: "GET".to_string(),
        url: "ftp://api.github.com".to_string(),
        headers: vec![],
        body: None,
    };
    let frame = capability_frame(1, &params);
    let result = connect_and_send(&paths, frame).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "scheme_not_allowed");
}

#[tokio::test]
async fn test_http_invalid_method() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);

    let runtime_paths = paths.clone();
    tokio::spawn(async move {
        prepare_core_runtime(&runtime_paths).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let params = HttpRequestParams {
        method: "TRACE".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    };
    let frame = capability_frame(1, &params);
    let result = connect_and_send(&paths, frame).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p kunkka-core --test http_runtime`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-core/tests/http_runtime.rs
git commit -m "test: add http capability runtime integration tests"
```

---

### Task 6: Run full verification

**Covers:** All

- [ ] **Step 1: Run formatting check**

Run: `cargo fmt --all --check`
Expected: PASS

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Update development log**

Append to `docs/development-log.md`:
```markdown
## 2026-06-18

### HTTP Capability (External API Request)

Implemented:

- `HttpRequestParams`/`HttpResponse` protocol types with postcard codec in `capability/http.rs`.
- App manifest `capabilities.http.domains` domain whitelist field with validation.
- Domain whitelist matching: exact match, case-insensitive, `http` and `https` schemes only.
- HTTP client with reqwest: fixed 30s timeout, auto-redirect (max 10), gzip/deflate compression, HTTP/1.1 + HTTP/2 auto-negotiation.
- Error codes: `invalid_params`, `permission_denied`, `scheme_not_allowed`, `timeout`, `io_error`.
- Tests: manifest loading (3), unit tests (7), runtime integration tests (4).

Verification:

```text
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
```

- [ ] **Step 5: Update architecture doc**

In `docs/architecture.md`, add to the capability layer section:
```markdown
- `capability/http.rs`：HTTP capability for external API requests with domain whitelist.
```

- [ ] **Step 6: Final commit**

```bash
git add docs/development-log.md docs/architecture.md
git commit -m "docs: update development log and architecture for http capability"
```
