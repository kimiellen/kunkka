# CLI Approval Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `kunkka-cli` 增加 shell approval 消费链路的最小前端闭环，让本地前端可以查询 pending approvals，并对指定 approval 做 approve / reject。

**Architecture:** 复用现有 `kunkka-cli -> kunkka.core-control.v1 -> kunkka-core` 通路，不引入新协议。CLI 新增 control 子命令，把 `ListPendingApprovals` / `ApprovePendingApproval` / `RejectPendingApproval` 映射到 JSON 输出；先把最小可脚本化入口落在 CLI，不在这一刀扩 TUI 或 native-host。

**Tech Stack:** Rust, clap, tokio, serde, serde_json, kunkka-ipc, kunkka-protocol, kunkka-core, tempfile

---

## File Structure

```text
docs/superpowers/plans/
└── 2026-06-17-cli-approval-control.md      # Create: this plan

crates/kunkka-cli/
├── src/cli.rs                              # Modify: add approval subcommands
├── src/client.rs                           # Modify: map new commands to core-control messages
├── src/lib.rs                              # Modify: handle approval commands
├── src/output.rs                           # Modify: add approval JSON result types
└── tests/
    ├── cli_args.rs                         # Modify: parse approval subcommands
    ├── client_mapping.rs                   # Modify: command -> core-control mapping
    ├── integration.rs                      # Modify: runtime approval flow integration tests
    └── output.rs                           # Modify: approval result JSON shape tests
```

---

### Task 1: CLI args and client mapping

**Files:**
- Modify: `crates/kunkka-cli/src/cli.rs`
- Modify: `crates/kunkka-cli/src/client.rs`
- Modify: `crates/kunkka-cli/tests/cli_args.rs`
- Modify: `crates/kunkka-cli/tests/client_mapping.rs`

- [ ] **Step 1: Write failing CLI parse tests**

Add tests for:

```rust
Cli::try_parse_from(["kunkka", "approvals", "list"])
Cli::try_parse_from(["kunkka", "approvals", "approve", "--id", "appr_1"])
Cli::try_parse_from(["kunkka", "approvals", "reject", "--id", "appr_1"])
```

Also add a failure case for empty `--id`.

- [ ] **Step 2: Write failing command mapping tests**

Assert that the new CLI commands map to:

```rust
CoreControlMessage::ListPendingApprovals(_)
CoreControlMessage::ApprovePendingApproval(_)
CoreControlMessage::RejectPendingApproval(_)
```

- [ ] **Step 3: Run focused tests to verify RED**

Run: `cargo test -p kunkka-cli --test cli_args --test client_mapping`

- [ ] **Step 4: Implement minimal CLI subcommands and mapping**

Add `approvals list|approve|reject` subcommands in `cli.rs`, and extend `core_message_for_command()` in `client.rs`.

- [ ] **Step 5: Re-run focused tests to verify GREEN**

Run: `cargo test -p kunkka-cli --test cli_args --test client_mapping`

---

### Task 2: CLI output and command execution

**Files:**
- Modify: `crates/kunkka-cli/src/lib.rs`
- Modify: `crates/kunkka-cli/src/output.rs`
- Modify: `crates/kunkka-cli/tests/output.rs`

- [ ] **Step 1: Write failing output tests**

Add JSON shape tests for:

```rust
CliResult::PendingApprovals { approvals: vec![...] }
CliResult::ApprovalDecision
```

- [ ] **Step 2: Run focused output test to verify RED**

Run: `cargo test -p kunkka-cli --test output`

- [ ] **Step 3: Implement minimal result types and `run_command_with_socket()` branches**

Behavior:
- `approvals list` returns `{"ok":true,"result":{"type":"pending_approvals",...}}`
- `approvals approve --id <id>` returns `{"ok":true,"result":{"type":"approval_decision"}}`
- `approvals reject --id <id>` returns the same decision result shape

- [ ] **Step 4: Re-run focused output test to verify GREEN**

Run: `cargo test -p kunkka-cli --test output`

---

### Task 3: Runtime integration coverage

**Files:**
- Modify: `crates/kunkka-cli/tests/integration.rs`

- [ ] **Step 1: Write failing integration tests**

Add tests that:
- create a shell `ask` pending approval through runtime
- call CLI `approvals list` and assert the pending item is visible
- call CLI `approvals approve --id ...` or `approvals reject --id ...` and assert worker retry sees the expected outcome

- [ ] **Step 2: Run focused integration test to verify RED**

Run: `cargo test -p kunkka-cli --test integration cli_approvals`

- [ ] **Step 3: Implement only what the integration tests still need**

Keep changes inside existing CLI modules; do not add a new client abstraction unless tests force it.

- [ ] **Step 4: Re-run focused integration test to verify GREEN**

Run: `cargo test -p kunkka-cli --test integration cli_approvals`

---

### Task 4: Verification

- [ ] Run: `cargo fmt --all --check`
- [ ] Run: `cargo test --workspace`
- [ ] Run: `cargo clippy --workspace --all-targets -- -D warnings`

---

### Task 5: Commit

- [ ] Commit after user approval.
