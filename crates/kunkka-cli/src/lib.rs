pub mod cli;
pub mod client;
pub mod error;
pub mod output;

use std::env;
use std::path::PathBuf;

use cli::{Cli, CliCommand};
use client::{
    build_frontend_dispatch_request, core_message_for_command, send_core_control,
    send_frontend_dispatch,
};
use error::CliError;
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

pub async fn run_command(cli: &Cli) -> Result<CliOutput, CliError> {
    let socket_path = resolve_socket_path()?;
    run_command_with_socket(cli, &socket_path).await
}

pub async fn run_command_with_socket(
    cli: &Cli,
    socket_path: &std::path::Path,
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
                    Ok(CliOutput::success(CliResult::DispatchError { code, message }))
                }
                FrontendDispatchResponse::PlatformError { code, message } => {
                    Err(CliError::CorePlatform { code, message })
                }
            }
        }
    }
}
