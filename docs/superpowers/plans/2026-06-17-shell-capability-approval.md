# Shell Capability Approval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Kunkka 增加受限 shell 子集执行能力，支持 manifest `allow/ask/deny` 命令策略与 core 侧 approval 协议。

**Architecture:** worker 继续通过 `kunkka.capability.v1` 走短连接 capability 请求，但 `shell.run` 先经过受限管道解析与 policy 判定。命中 `ask` 时，core 创建内存中的 pending approval，并通过扩展 `kunkka.core-control.v1` 让 CLI、TUI、浏览器插件前端后续都能查询、批准或拒绝；批准后 worker 携带 `approval_id` 重试同一命令再执行。pending approval 的 TTL 固定为 1 分钟，第一刀通过 approval store 的懒清理完成过期与回收，不在 runtime 主循环里新增周期任务。

**Tech Stack:** Rust, Tokio, postcard, serde, `std::process::Command`, kunkka-ipc, kunkka-protocol, tempfile

---

## File Structure

```text
crates/kunkka-protocol/
├── src/core_control.rs                    # Modify: add approval control messages
└── tests/core_control.rs                  # Modify: roundtrip tests for approval messages

crates/kunkka-core/
├── src/app_manifest.rs                    # Modify: add capabilities.shell config
├── src/approval.rs                        # Create: pending approval store and lifecycle
├── src/capability/mod.rs                  # Modify: route shell capability
├── src/capability/shell.rs                # Create: shell params, parser, policy, execution
├── src/capability/permissions.rs          # Modify: add shell policy decision helpers
├── src/lib.rs                             # Modify: export approval module
├── src/permissions.rs                     # Keep frontend dispatch static allow/deny only
├── src/runtime.rs                         # Modify: handle approval control messages
└── tests/
    ├── app_manifest.rs                    # Modify: shell manifest parsing/validation tests
    ├── shell_policy.rs                    # Create: parser + policy unit tests
    └── shell_runtime.rs                   # Create: allow/ask/deny integration tests

crates/kunkka-cli/
└── tests/integration.rs                   # Modify: minimal approval protocol smoke test if needed
```

---

### Task 1: Manifest and Protocol Types

**Covers:** [S2, S4, S5]

**Files:**
- Modify: `crates/kunkka-core/src/app_manifest.rs`
- Modify: `crates/kunkka-core/tests/app_manifest.rs`
- Modify: `crates/kunkka-protocol/src/core_control.rs`
- Modify: `crates/kunkka-protocol/tests/core_control.rs`

- [ ] **Step 1: Write failing manifest tests for shell capability**

In `crates/kunkka-core/tests/app_manifest.rs`, add tests shaped like:

```rust
#[test]
fn manifest_loads_shell_allow_and_ask_lists() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(
        &path,
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []},
            "capabilities": {
                "shell": {
                    "allow": ["rg", "wc"],
                    "ask": ["curl"]
                }
            }
        }"#,
    )
    .unwrap();

    let manifest = kunkka_core::app_manifest::AppManifest::load_file(&path).unwrap();
    let shell = manifest.capabilities.shell.unwrap();
    assert_eq!(shell.allow, vec!["rg", "wc"]);
    assert_eq!(shell.ask, vec!["curl"]);
}

#[test]
fn manifest_rejects_command_present_in_allow_and_ask() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(
        &path,
        r#"{
            "app_id": "notes",
            "worker": {"program": "/usr/bin/notes", "args": []},
            "capabilities": {
                "shell": {
                    "allow": ["curl"],
                    "ask": ["curl"]
                }
            }
        }"#,
    )
    .unwrap();

    let err = kunkka_core::app_manifest::AppManifest::load_file(&path).unwrap_err();
    assert!(err.to_string().contains("allow and ask"));
}
```

- [ ] **Step 2: Run manifest tests to verify RED**

Run: `cargo test -p kunkka-core --test app_manifest`
Expected: FAIL with missing `capabilities.shell` support

- [ ] **Step 3: Add shell config types to manifest**

In `crates/kunkka-core/src/app_manifest.rs`, add the minimal types:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShellCapabilityConfig {
    pub allow: Vec<String>,
    pub ask: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawShellCapabilityConfig {
    #[serde(default)]
    allow: Option<Vec<String>>,
    #[serde(default)]
    ask: Option<Vec<String>>,
}
```

And extend `CapabilitiesConfig` / `RawCapabilitiesConfig` with:

```rust
pub struct CapabilitiesConfig {
    pub fs: Option<FsCapabilityConfig>,
    pub shell: Option<ShellCapabilityConfig>,
}
```

Also validate that shell command names are non-empty and do not appear in both `allow` and `ask`.

- [ ] **Step 4: Add failing protocol roundtrip test for approval control**

In `crates/kunkka-protocol/tests/core_control.rs`, add a roundtrip test like:

```rust
#[test]
fn approval_list_payload_roundtrips() {
    let payload = kunkka_protocol::core_control::encode_control_message(
        &kunkka_protocol::core_control::CoreControlMessage::PendingApprovalsResult(
            kunkka_protocol::core_control::CoreListApprovalsResponse {
                approvals: vec![kunkka_protocol::core_control::PendingApproval {
                    approval_id: "appr_1".to_string(),
                    app_id: "notes".to_string(),
                    capability: "shell".to_string(),
                    summary: "curl https://example.com".to_string(),
                }],
            },
        ),
    )
    .unwrap();

    let decoded = kunkka_protocol::core_control::decode_control_message(&payload).unwrap();
    assert!(matches!(decoded, kunkka_protocol::core_control::CoreControlMessage::PendingApprovalsResult(_)));
}
```

- [ ] **Step 5: Run protocol tests to verify RED**

Run: `cargo test -p kunkka-protocol --test core_control`
Expected: FAIL with missing approval control types

- [ ] **Step 6: Extend core control protocol messages minimally**

In `crates/kunkka-protocol/src/core_control.rs`, extend `CoreControlMessage` with:

```rust
ListPendingApprovals(CoreListApprovalsRequest),
PendingApprovalsResult(CoreListApprovalsResponse),
ApprovePendingApproval(CoreApproveApprovalRequest),
RejectPendingApproval(CoreRejectApprovalRequest),
ApprovalDecisionResult(CoreApprovalDecisionResponse),
```

and add the structs used by the tests.

- [ ] **Step 7: Run focused tests to verify GREEN**

Run: `cargo test -p kunkka-core --test app_manifest && cargo test -p kunkka-protocol --test core_control`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/kunkka-core/src/app_manifest.rs crates/kunkka-core/tests/app_manifest.rs crates/kunkka-protocol/src/core_control.rs crates/kunkka-protocol/tests/core_control.rs
git commit -m "feat: add shell policy and approval protocol types"
```

---

### Task 2: Restricted Shell Parser and Policy Engine

**Covers:** [S3, S4, S6]

**Files:**
- Create: `crates/kunkka-core/src/capability/shell.rs`
- Modify: `crates/kunkka-core/src/capability/mod.rs`
- Modify: `crates/kunkka-core/src/capability/permissions.rs`
- Create: `crates/kunkka-core/tests/shell_policy.rs`

- [ ] **Step 1: Write failing parser/policy tests**

In `crates/kunkka-core/tests/shell_policy.rs`, add tests like:

```rust
#[test]
fn parses_top_level_pipeline_commands() {
    let stages = kunkka_core::capability::shell::parse_pipeline("rg todo src | wc -l").unwrap();
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].command, "rg");
    assert_eq!(stages[1].command, "wc");
}

#[test]
fn keeps_pipe_inside_quotes() {
    let stages = kunkka_core::capability::shell::parse_pipeline("echo 'a|b' | rg a").unwrap();
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].command, "echo");
    assert_eq!(stages[1].command, "rg");
}

#[test]
fn rejects_redirects_and_and_operators() {
    assert!(kunkka_core::capability::shell::parse_pipeline("rg todo > out.txt").is_err());
    assert!(kunkka_core::capability::shell::parse_pipeline("rg todo && wc -l").is_err());
}
```

Also add decision tests for allow/ask/deny aggregation.

- [ ] **Step 2: Run policy tests to verify RED**

Run: `cargo test -p kunkka-core --test shell_policy`
Expected: FAIL with missing shell capability module

- [ ] **Step 3: Implement minimal parser and decision helpers**

In `crates/kunkka-core/src/capability/shell.rs`, add the core types:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRunParams {
    pub command: String,
    pub approval_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStage {
    pub command: String,
}

pub fn parse_pipeline(input: &str) -> Result<Vec<ParsedStage>, CapabilityError> { /* minimal top-level pipeline parser */ }
```

In `crates/kunkka-core/src/capability/permissions.rs`, add a three-state decision helper over shell command names:

```rust
pub enum ShellPolicyDecision {
    Allow,
    Ask,
    Deny,
}

pub fn decide_shell_policy(manifest: &AppManifest, commands: &[String]) -> ShellPolicyDecision {
    /* any deny => deny; any ask => ask; all allow => allow */
}
```

- [ ] **Step 4: Export shell capability module**

In `crates/kunkka-core/src/capability/mod.rs`, add `pub mod shell;` and re-export the shell result types needed by tests.

- [ ] **Step 5: Run policy tests to verify GREEN**

Run: `cargo test -p kunkka-core --test shell_policy`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-core/src/capability/mod.rs crates/kunkka-core/src/capability/permissions.rs crates/kunkka-core/src/capability/shell.rs crates/kunkka-core/tests/shell_policy.rs
git commit -m "feat: add restricted shell parser and policy engine"
```

---

### Task 3: Approval Store and Core Runtime Wiring

**Covers:** [S2, S5, S6]

**Files:**
- Create: `crates/kunkka-core/src/approval.rs`
- Modify: `crates/kunkka-core/src/lib.rs`
- Modify: `crates/kunkka-core/src/runtime.rs`
- Modify: `crates/kunkka-core/src/capability/shell.rs`

- [ ] **Step 1: Write failing approval lifecycle tests**

Add tests in `crates/kunkka-core/tests/shell_runtime.rs` for the lifecycle:

```rust
#[tokio::test]
async fn ask_returns_pending_approval_receipt() {
    let response = run_shell_command_with_manifest("curl https://example.com", manifest_with_ask()).await;
    assert!(matches!(response, ShellRunOutcome::PendingApproval(_)));
}

#[tokio::test]
async fn approved_receipt_can_be_retried_once() {
    let receipt = request_pending_approval().await;
    approve_pending(&receipt.approval_id).await;
    let result = retry_shell_with_approval_id("curl https://example.com", &receipt.approval_id).await;
    assert!(matches!(result, ShellRunOutcome::Completed(_)));
}
```

- [ ] **Step 2: Run runtime tests to verify RED**

Run: `cargo test -p kunkka-core --test shell_runtime`
Expected: FAIL with missing approval store / runtime control handlers

- [ ] **Step 3: Implement in-memory approval store**

Create `crates/kunkka-core/src/approval.rs` with a minimal store:

```rust
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

pub struct PendingApprovalEntry {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub command: String,
    pub commands: Vec<String>,
    pub state: ApprovalState,
}

pub struct ApprovalStore {
    entries: std::collections::BTreeMap<String, PendingApprovalEntry>,
}
```

- [ ] **Step 4: Wire shell.run to approval receipts and retry checks**

In `crates/kunkka-core/src/capability/shell.rs`, implement:

```rust
pub enum ShellRunOutcome {
    Completed(ShellRunResult),
    PendingApproval(PendingApprovalReceipt),
}

pub async fn handle_shell_request(/* manifest, params, approvals */) -> Result<Vec<u8>, CapabilityError> {
    /* parse -> decide -> deny | create receipt | validate approved receipt and execute */
}
```

- [ ] **Step 5: Extend runtime for approval control messages**

In `crates/kunkka-core/src/runtime.rs`, handle new `CoreControlMessage` variants by listing approvals and applying approve/reject transitions.

- [ ] **Step 6: Run runtime tests to verify GREEN**

Run: `cargo test -p kunkka-core --test shell_runtime`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/kunkka-core/src/approval.rs crates/kunkka-core/src/lib.rs crates/kunkka-core/src/runtime.rs crates/kunkka-core/src/capability/shell.rs crates/kunkka-core/tests/shell_runtime.rs
git commit -m "feat: add shell approval lifecycle to core runtime"
```

---

### Task 4: End-to-End Verification

**Covers:** [S1, S2, S5, S6, S7]

**Files:**
- Test: `crates/kunkka-core/tests/shell_runtime.rs`
- Test: `crates/kunkka-protocol/tests/core_control.rs`
- Test: `crates/kunkka-core/tests/shell_policy.rs`

- [ ] **Step 1: Add explicit allow/deny/ask integration coverage**

Ensure `crates/kunkka-core/tests/shell_runtime.rs` includes these cases:

```rust
#[tokio::test]
async fn allow_executes_simple_pipeline() {
    let result = run_allowed_shell("echo foo | wc -c").await;
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn deny_rejects_unlisted_command() {
    let err = run_denied_shell("python -c 'print(1)'").await.unwrap_err();
    assert!(err.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn expired_approval_cannot_be_consumed() {
    let receipt = request_pending_approval().await;
    expire_pending(&receipt.approval_id).await;
    let err = retry_shell_with_approval_id("curl https://example.com", &receipt.approval_id)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("approval_denied"));
}
```

- [ ] **Step 2: Run focused shell verification**

Run: `cargo test -p kunkka-core --test app_manifest --test shell_policy --test shell_runtime && cargo test -p kunkka-protocol --test core_control`
Expected: PASS

- [ ] **Step 3: Run workspace verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/kunkka-core crates/kunkka-protocol docs/superpowers/specs/2026-06-17-shell-capability-approval-design.md docs/superpowers/plans/2026-06-17-shell-capability-approval.md
git commit -m "feat: add shell capability approval flow"
```

---

### Task 5: Approval TTL and Lazy Cleanup

**Covers:** [S5, S6, S7]

**Files:**
- Modify: `crates/kunkka-core/src/approval.rs`
- Modify: `crates/kunkka-core/tests/shell_runtime.rs`
- Modify: `docs/superpowers/specs/2026-06-17-shell-capability-approval-design.md`

- [ ] **Step 1: Write the failing expiration tests**

In `crates/kunkka-core/tests/shell_runtime.rs`, add coverage shaped like:

```rust
#[tokio::test]
async fn expired_pending_approval_is_hidden_from_list() {
    let receipt = request_pending_approval().await;
    expire_pending_for_test(&receipt.approval_id).await;
    let approvals = list_pending_approvals().await;
    assert!(approvals.iter().all(|approval| approval.approval_id != receipt.approval_id));
}

#[tokio::test]
async fn expired_pending_approval_cannot_be_consumed() {
    let receipt = request_pending_approval().await;
    expire_pending_for_test(&receipt.approval_id).await;
    let err = retry_shell_with_approval_id("printf later", &receipt.approval_id)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("approval_denied"));
}
```

- [ ] **Step 2: Run runtime tests to verify RED**

Run: `cargo test -p kunkka-core --test shell_runtime`
Expected: FAIL because approval store has no TTL/lazy cleanup behavior yet

- [ ] **Step 3: Add created_at and TTL to approval store**

In `crates/kunkka-core/src/approval.rs`, extend the store with minimal timing data:

```rust
use std::time::{Duration, Instant};

const APPROVAL_TTL: Duration = Duration::from_secs(60);

pub struct ApprovalRecord {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub command: String,
    pub commands: Vec<String>,
    pub state: ApprovalState,
    pub created_at: Instant,
}
```

And ensure `create()` stamps `Instant::now()`.

- [ ] **Step 4: Implement lazy cleanup in approval store**

In `crates/kunkka-core/src/approval.rs`, add one internal helper and call it from `list_pending`, `approve`, `reject`, and `consume_approved`:

```rust
fn reap_expired(&mut self, now: Instant) {
    for entry in self.entries.values_mut() {
        if entry.state == ApprovalState::Pending
            && now.duration_since(entry.created_at) >= APPROVAL_TTL
        {
            entry.state = ApprovalState::Expired;
        }
    }

    self.entries.retain(|_, entry| {
        !matches!(entry.state, ApprovalState::Rejected | ApprovalState::Expired)
    });
}
```

Also add a minimal test-only hook so runtime tests can deterministically age a pending approval without sleeping for a minute.

- [ ] **Step 5: Run focused tests to verify GREEN**

Run: `cargo test -p kunkka-core --test shell_runtime`
Expected: PASS

- [ ] **Step 6: Refresh focused verification**

Run: `cargo fmt --all --check && cargo test -p kunkka-core --test app_manifest --test shell_policy --test shell_runtime && cargo test -p kunkka-protocol --test core_control`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/kunkka-core/src/approval.rs crates/kunkka-core/tests/shell_runtime.rs docs/superpowers/specs/2026-06-17-shell-capability-approval-design.md docs/superpowers/plans/2026-06-17-shell-capability-approval.md
git commit -m "feat: expire pending shell approvals lazily"
```
