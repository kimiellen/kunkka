# CLI Shell Approval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `kunkka-cli` 增加 shell capability 的交互式 approval 消费流程，让一次 CLI 命令内可完成请求、确认、批准/拒绝与重试执行。

**Architecture:** CLI 新增 `shell` 子命令并继续使用短连接模式。命中 `PendingApproval` 时，CLI 立即走 `kunkka.core-control.v1` 查询待审批项并在终端提示用户确认；批准后带同一 `approval_id` 重试原 `shell.run` 请求，拒绝则直接返回本地错误。

**Tech Stack:** Rust, clap, Tokio, kunkka-ipc, kunkka-protocol, tempfile, postcard

---

## File Structure

```text
crates/kunkka-cli/
├── src/
│   ├── cli.rs               # Modify: add shell subcommand
│   ├── client.rs            # Modify: add shell capability + approval control helpers
│   ├── error.rs             # Modify: add local approval rejection / missing approval errors
│   ├── lib.rs               # Modify: orchestrate shell request -> prompt -> approve/reject -> retry
│   └── output.rs            # Modify: format shell execution output if needed
└── tests/
    ├── cli_args.rs          # Modify: shell subcommand parsing
    └── integration.rs       # Modify: end-to-end shell approval flows
```

---

### Task 1: Shell CLI Surface

**Covers:** [S2, S6, S7]

**Files:**
- Modify: `crates/kunkka-cli/src/cli.rs`
- Modify: `crates/kunkka-cli/tests/cli_args.rs`

- [ ] **Step 1: Write failing CLI parsing test**

In `crates/kunkka-cli/tests/cli_args.rs`, add:

```rust
#[test]
fn parses_shell_command() {
    let cli = kunkka_cli::cli::Cli::try_parse_from([
        "kunkka",
        "shell",
        "--app",
        "notes",
        "--command",
        "printf foo | wc -c",
    ])
    .unwrap();

    match cli.command {
        kunkka_cli::cli::CliCommand::Shell { app_id, command } => {
            assert_eq!(app_id, "notes");
            assert_eq!(command, "printf foo | wc -c");
        }
        other => panic!("expected shell command, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run parsing test to verify RED**

Run: `cargo test -p kunkka-cli --test cli_args parses_shell_command`
Expected: FAIL because `CliCommand::Shell` does not exist yet

- [ ] **Step 3: Add minimal shell subcommand**

In `crates/kunkka-cli/src/cli.rs`, add:

```rust
Shell {
    #[arg(long = "app", value_parser = validate_non_empty)]
    app_id: String,
    #[arg(long, value_parser = validate_non_empty)]
    command: String,
},
```

- [ ] **Step 4: Run parsing test to verify GREEN**

Run: `cargo test -p kunkka-cli --test cli_args parses_shell_command`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-cli/src/cli.rs crates/kunkka-cli/tests/cli_args.rs
git commit -m "feat: add cli shell subcommand"
```

---

### Task 2: Capability and Approval Client Helpers

**Covers:** [S3, S4, S5, S6]

**Files:**
- Modify: `crates/kunkka-cli/src/client.rs`
- Modify: `crates/kunkka-cli/src/error.rs`

- [ ] **Step 1: Write failing helper-level tests or call-site tests**

Add tests in `crates/kunkka-cli/tests/integration.rs` that expect these flows to exist:

```rust
#[tokio::test]
async fn cli_shell_pending_can_be_approved_and_retried() {
    let result = run_cli_shell_with_confirmation("printf approved", "y\n").await;
    assert!(result.stdout.contains("approved"));
}

#[tokio::test]
async fn cli_shell_pending_can_be_rejected() {
    let err = run_cli_shell_with_confirmation("printf denied", "n\n").await.unwrap_err();
    assert!(err.to_string().contains("approval rejected by user"));
}
```

- [ ] **Step 2: Run targeted integration test to verify RED**

Run: `cargo test -p kunkka-cli --test integration cli_shell_pending_can_be_approved_and_retried`
Expected: FAIL because shell client/orchestration helpers do not exist yet

- [ ] **Step 3: Add minimal client helpers**

In `crates/kunkka-cli/src/client.rs`, add helpers shaped like:

```rust
pub async fn send_shell_run(
    socket_path: &Path,
    app_id: String,
    command: String,
    approval_id: Option<String>,
) -> Result<kunkka_core::capability::shell::ShellRunOutcome, CliError> { /* short connection */ }

pub async fn list_pending_approvals(
    socket_path: &Path,
) -> Result<kunkka_protocol::core_control::CoreListApprovalsResponse, CliError> { /* core control */ }

pub async fn approve_pending_approval(
    socket_path: &Path,
    approval_id: String,
) -> Result<(), CliError> { /* core control */ }

pub async fn reject_pending_approval(
    socket_path: &Path,
    approval_id: String,
) -> Result<(), CliError> { /* core control */ }
```

Also add a local CLI error for user rejection / missing approval lookup.

- [ ] **Step 4: Run targeted integration test to verify helper layer is GREEN-ready**

Run: `cargo test -p kunkka-cli --test integration cli_shell_pending_can_be_approved_and_retried`
Expected: Still FAIL at orchestration layer, but no longer because low-level helpers are missing

- [ ] **Step 5: Commit**

```bash
git add crates/kunkka-cli/src/client.rs crates/kunkka-cli/src/error.rs
git commit -m "feat: add cli shell approval client helpers"
```

---

### Task 3: Interactive Approval Flow

**Covers:** [S1, S3, S5, S6, S7]

**Files:**
- Modify: `crates/kunkka-cli/src/lib.rs`
- Modify: `crates/kunkka-cli/src/output.rs`
- Modify: `crates/kunkka-cli/tests/integration.rs`

- [ ] **Step 1: Extend failing integration coverage for all CLI paths**

Ensure `crates/kunkka-cli/tests/integration.rs` covers:

```rust
#[tokio::test]
async fn cli_shell_completed_outputs_result() {
    let result = run_cli_shell_with_confirmation("printf ok", "").await.unwrap();
    assert!(result.stdout.contains("ok"));
}

#[tokio::test]
async fn cli_shell_pending_can_be_approved_and_retried() {
    let result = run_cli_shell_with_confirmation("printf approved", "y\n").await.unwrap();
    assert!(result.stdout.contains("approved"));
}

#[tokio::test]
async fn cli_shell_pending_can_be_rejected() {
    let err = run_cli_shell_with_confirmation("printf denied", "n\n").await.unwrap_err();
    assert!(err.to_string().contains("approval rejected by user"));
}

#[tokio::test]
async fn cli_shell_expired_approval_reports_error() {
    let err = run_cli_shell_with_expired_approval("printf late", "y\n").await.unwrap_err();
    assert!(err.to_string().contains("approval_denied"));
}
```

- [ ] **Step 2: Run integration tests to verify RED**

Run: `cargo test -p kunkka-cli --test integration`
Expected: FAIL because interactive orchestration is not implemented yet

- [ ] **Step 3: Implement minimal orchestration in lib.rs**

In `crates/kunkka-cli/src/lib.rs`, add a flow shaped like:

```rust
match send_shell_run(socket_path, app_id.clone(), command.clone(), None).await? {
    ShellRunOutcome::Completed(result) => render_shell_result(result),
    ShellRunOutcome::PendingApproval(receipt) => {
        let approvals = list_pending_approvals(socket_path).await?;
        let pending = approvals
            .approvals
            .into_iter()
            .find(|item| item.approval_id == receipt.approval_id)
            .ok_or_else(|| CliError::ApprovalNotFound(receipt.approval_id.clone()))?;

        if confirm_pending_approval(&pending)? {
            approve_pending_approval(socket_path, receipt.approval_id.clone()).await?;
            let outcome = send_shell_run(socket_path, app_id, command, Some(receipt.approval_id)).await?;
            /* expect Completed and render */
        } else {
            reject_pending_approval(socket_path, receipt.approval_id).await?;
            return Err(CliError::ApprovalRejectedByUser);
        }
    }
}
```

Use the smallest possible stdin-based confirmation parser: `y`/`yes` approve, anything else reject.

- [ ] **Step 4: Run integration tests to verify GREEN**

Run: `cargo test -p kunkka-cli --test integration`
Expected: PASS

- [ ] **Step 5: Run full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/kunkka-cli/src/lib.rs crates/kunkka-cli/src/output.rs crates/kunkka-cli/tests/integration.rs docs/superpowers/specs/2026-06-17-cli-shell-approval-design.md docs/superpowers/plans/2026-06-17-cli-shell-approval.md
git commit -m "feat: add cli shell approval flow"
```
