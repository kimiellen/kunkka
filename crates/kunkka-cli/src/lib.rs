pub mod cli;
pub mod client;
pub mod error;
pub mod output;

use std::env;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

use cli::{ApprovalCommand, Cli, CliCommand};
use client::{
    approve_pending_approval, build_frontend_dispatch_request, core_message_for_command,
    list_pending_approvals, reject_pending_approval, send_core_control, send_frontend_dispatch,
    send_shell_request,
};
use error::CliError;
use kunkka_protocol::core_control::CoreControlMessage;
use kunkka_protocol::frontend_dispatch::FrontendDispatchResponse;
use output::{CliOutput, CliResult};

/// Resolve the Kunkka core socket path using XDG conventions.
///
/// This mirrors the logic in `kunkka-core::xdg::KunkkaPaths` but is kept
/// minimal so that `kunkka-cli` does not depend on `kunkka-core` at runtime.
pub fn resolve_socket_path() -> Result<PathBuf, error::CliError> {
    let _home = env_path("HOME")
        .ok_or_else(|| error::CliError::CoreUnavailable("HOME is not set".to_string()))?;

    let runtime_dir = absolute_env_path(&env_path("XDG_RUNTIME_DIR"))
        .map(|path| path.join("kunkka"))
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/kunkka-runtime-{}", effective_uid())));

    Ok(runtime_dir.join("core.sock"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn absolute_env_path(path: &Option<PathBuf>) -> Option<PathBuf> {
    path.as_ref().filter(|path| path.is_absolute()).cloned()
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() as u32 }
}

fn confirm_approval_prompt(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

pub async fn run_command(cli: &Cli) -> Result<CliOutput, CliError> {
    let socket_path = resolve_socket_path()?;
    run_command_with_socket(cli, &socket_path).await
}

pub async fn run_command_with_socket(
    cli: &Cli,
    socket_path: &std::path::Path,
) -> Result<CliOutput, CliError> {
    run_command_with_socket_and_input(cli, socket_path, std::io::stdin()).await
}

pub async fn run_command_with_socket_and_input(
    cli: &Cli,
    socket_path: &std::path::Path,
    input: impl Read,
) -> Result<CliOutput, CliError> {
    match &cli.command {
        CliCommand::Ping => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::Pong(_) => {
                    Ok(CliOutput::success(CliResult::Pong))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected pong, got {other:?}"
                ))),
            }
        }
        CliCommand::Status => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(socket_path, message).await?;
            match response {
                kunkka_protocol::core_control::CoreControlMessage::StatusResult(status) => {
                    Ok(CliOutput::success(CliResult::Status {
                        worker_count: status.worker_count,
                        socket_path: status.socket_path,
                        runtime_ready: status.runtime_ready,
                    }))
                }
                other => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected status result, got {other:?}"
                ))),
            }
        }
        CliCommand::Approvals { command } => {
            let message = core_message_for_command(&cli.command)
                .ok_or_else(|| CliError::InvalidRequest("expected control command".to_string()))?;
            let response = send_core_control(socket_path, message).await?;
            match (command, response) {
                (ApprovalCommand::List, CoreControlMessage::PendingApprovalsResult(result)) => {
                    Ok(CliOutput::success(CliResult::PendingApprovals {
                        approvals: result.approvals,
                    }))
                }
                (
                    ApprovalCommand::Approve { .. } | ApprovalCommand::Reject { .. },
                    CoreControlMessage::ApprovalDecisionResult(_),
                ) => Ok(CliOutput::success(CliResult::ApprovalDecision)),
                (ApprovalCommand::List, other) => Err(CliError::UnexpectedCoreResponse(format!(
                    "expected pending approvals result, got {other:?}"
                ))),
                (ApprovalCommand::Approve { .. }, other) => Err(CliError::UnexpectedCoreResponse(
                    format!("expected approval decision result, got {other:?}"),
                )),
                (ApprovalCommand::Reject { .. }, other) => Err(CliError::UnexpectedCoreResponse(
                    format!("expected approval decision result, got {other:?}"),
                )),
            }
        }
        CliCommand::Shell { app_id, command } => {
            match send_shell_request(socket_path, app_id.clone(), command.clone(), None).await? {
                client::ShellRunOutcome::Completed(result) => {
                    Ok(CliOutput::success(CliResult::ShellResult {
                        stdout: result.stdout,
                        stderr: result.stderr,
                        exit_code: result.exit_code,
                    }))
                }
                client::ShellRunOutcome::PendingApproval(receipt) => {
                    handle_shell_approval(
                        socket_path,
                        app_id.clone(),
                        command.clone(),
                        &receipt.approval_id,
                        input,
                    )
                    .await
                }
            }
        }
        CliCommand::Dispatch {
            app_id,
            method,
            payload,
        } => {
            let request =
                build_frontend_dispatch_request(app_id.clone(), method.clone(), payload.clone());
            let response = send_frontend_dispatch(socket_path, request).await?;
            match response {
                FrontendDispatchResponse::Ok(payload) => {
                    let value: serde_json::Value = serde_json::from_slice(&payload.bytes)
                        .map_err(|err| CliError::CoreIpc(format!("invalid JSON payload: {err}")))?;
                    Ok(CliOutput::success(CliResult::Dispatch { payload: value }))
                }
                FrontendDispatchResponse::AppError { code, message } => {
                    Ok(CliOutput::success(CliResult::DispatchError {
                        code,
                        message,
                    }))
                }
                FrontendDispatchResponse::PlatformError { code, message } => {
                    Err(CliError::CorePlatform { code, message })
                }
            }
        }
    }
}

async fn handle_shell_approval(
    socket_path: &std::path::Path,
    app_id: String,
    command: String,
    approval_id: &str,
    input: impl Read,
) -> Result<CliOutput, CliError> {
    let approvals = list_pending_approvals(socket_path).await?;
    let pending = approvals
        .into_iter()
        .find(|approval| approval.approval_id == approval_id)
        .ok_or_else(|| CliError::CorePlatform {
            code: "approval_missing".to_string(),
            message: format!("approval {approval_id} was not found in pending approvals"),
        })?;

    eprintln!("{}", pending.summary);
    eprint!("Approve? [y/N] ");

    let mut reader = BufReader::new(input);
    let mut line = String::new();
    let confirmed = match reader.read_line(&mut line) {
        Ok(0) | Err(_) => false,
        Ok(_) => confirm_approval_prompt(&line),
    };

    if confirmed {
        approve_pending_approval(socket_path, approval_id.to_string()).await?;
        match send_shell_request(socket_path, app_id, command, Some(approval_id.to_string()))
            .await?
        {
            client::ShellRunOutcome::Completed(result) => {
                Ok(CliOutput::success(CliResult::ShellResult {
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                }))
            }
            client::ShellRunOutcome::PendingApproval(_) => Err(CliError::UnexpectedCoreResponse(
                "shell request still pending after approval".to_string(),
            )),
        }
    } else {
        reject_pending_approval(socket_path, approval_id.to_string()).await?;
        Err(CliError::ApprovalRejected)
    }
}
