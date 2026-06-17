# Native Host Approval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `kunkka-native-host` 增加 approval 消费能力，让浏览器插件可通过 Native Messaging 协议查询、批准、拒绝 pending approvals。

**Architecture:** 新增 3 个 NativeCommand（ApprovalsList / ApprovalApprove / ApprovalReject），复用现有 `send_core_control` 桥接模式，通过 core-control 协议操作 approvals。

**Tech Stack:** Rust, serde_json, kunkka-ipc, kunkka-protocol, tempfile

---

## File Structure

```text
crates/kunkka-native-host/
├── src/
│   ├── native_protocol.rs  # Modify: NativeCommand/NativeResult/NativePendingApproval 扩展
│   ├── bridge.rs           # Modify: core_message_for_command/native_result_for_core_response/handle_request 扩展
│   └── host.rs             # Keep: 不变
└── tests/
    ├── bridge_mapping.rs   # Modify: approval 命令映射测试
    └── bridge_session.rs   # Modify: approval 会话测试
```

---

### Task 1: Protocol Types

**Covers:** [S2, S3]

**Files:**
- Modify: `crates/kunkka-native-host/src/native_protocol.rs`
- Modify: `crates/kunkka-native-host/tests/bridge_mapping.rs`

- [ ] **Step 1: Write failing protocol tests**

In `crates/kunkka-native-host/tests/bridge_mapping.rs`, add:

```rust
#[test]
fn approvals_list_command_decodes_from_json() {
    let request = decode_request(br#"{"id":"1","command":"approvals_list"}"#).unwrap();
    assert_eq!(request.command, NativeCommand::ApprovalsList);
}

#[test]
fn approval_approve_command_decodes_from_json() {
    let request = decode_request(br#"{"id":"2","command":"approval_approve","approval_id":"appr_1"}"#).unwrap();
    assert_eq!(request.command, NativeCommand::ApprovalApprove { approval_id: "appr_1".to_string() });
}

#[test]
fn pending_approvals_result_serializes_to_json() {
    let response = success_response("1", NativeResult::PendingApprovals {
        approvals: vec![NativePendingApproval {
            approval_id: "appr_1".to_string(),
            app_id: "notes".to_string(),
            capability: "shell".to_string(),
            summary: "printf hello".to_string(),
        }],
    });
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("pending_approvals"));
    assert!(json.contains("appr_1"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p kunkka-native-host --test bridge_mapping`
Expected: FAIL because `ApprovalsList` / `ApprovalApprove` / `ApprovalReject` do not exist yet

- [ ] **Step 3: Add protocol types**

In `crates/kunkka-native-host/src/native_protocol.rs`, add to `NativeCommand`:

```rust
ApprovalsList,
ApprovalApprove { approval_id: String },
ApprovalReject { approval_id: String },
```

Add to `NativeResult`:

```rust
PendingApprovals { approvals: Vec<NativePendingApproval> },
ApprovalDecision,
```

Add new struct:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePendingApproval {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p kunkka-native-host --test bridge_mapping`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-native-host/src/native_protocol.rs crates/kunkka-native-host/tests/bridge_mapping.rs
git commit -m "feat: add native host approval protocol types"
```

---

### Task 2: Bridge Handling

**Covers:** [S4, S5]

**Files:**
- Modify: `crates/kunkka-native-host/src/bridge.rs`
- Modify: `crates/kunkka-native-host/tests/bridge_session.rs`

- [ ] **Step 1: Write failing bridge tests**

In `crates/kunkka-native-host/tests/bridge_session.rs`, add:

```rust
#[tokio::test]
async fn session_handles_approvals_list_command() {
    // setup core runtime with pending approval
    // send approvals_list command
    // assert response contains pending approvals
}

#[tokio::test]
async fn session_handles_approval_approve_command() {
    // setup core runtime with pending approval
    // send approval_approve command
    // assert response is approval_decision
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p kunkka-native-host --test bridge_session`
Expected: FAIL because `core_message_for_command` doesn't handle approval commands yet

- [ ] **Step 3: Implement bridge handling**

In `crates/kunkka-native-host/src/bridge.rs`, extend:

1. `core_message_for_command`:
```rust
NativeCommand::ApprovalsList => Ok(CoreControlMessage::ListPendingApprovals(CoreListApprovalsRequest)),
NativeCommand::ApprovalApprove { approval_id } => Ok(CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest { approval_id: approval_id.clone() })),
NativeCommand::ApprovalReject { approval_id } => Ok(CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest { approval_id: approval_id.clone() })),
```

2. `native_result_for_core_response`:
```rust
(NativeCommand::ApprovalsList, CoreControlMessage::PendingApprovalsResult(result)) => {
    Ok(NativeResult::PendingApprovals {
        approvals: result.approvals.into_iter().map(|a| NativePendingApproval {
            approval_id: a.approval_id,
            app_id: a.app_id,
            capability: a.capability,
            summary: a.summary,
        }).collect(),
    })
}
(NativeCommand::ApprovalApprove { .. } | NativeCommand::ApprovalReject { .. }, CoreControlMessage::ApprovalDecisionResult(_)) => {
    Ok(NativeResult::ApprovalDecision)
}
```

3. `handle_request`: add 3 new match arms following the same pattern as Ping/Status

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p kunkka-native-host --test bridge_session`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-native-host/src/bridge.rs crates/kunkka-native-host/tests/bridge_session.rs
git commit -m "feat: add native host approval bridge handling"
```

---

### Task 3: End-to-End Verification

**Covers:** [S1, S2, S3, S4, S5]

**Files:**
- Test: `crates/kunkka-native-host/tests/bridge_mapping.rs`
- Test: `crates/kunkka-native-host/tests/bridge_session.rs`

- [ ] **Step 1: Run focused verification**

Run: `cargo test -p kunkka-native-host`
Expected: PASS

- [ ] **Step 2: Run workspace verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/kunkka-native-host crates/kunkka-protocol docs/superpowers/specs/2026-06-17-native-host-approval-design.md docs/superpowers/plans/2026-06-17-native-host-approval.md
git commit -m "feat: add native host approval consumption flow"
```
