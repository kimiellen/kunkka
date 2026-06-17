use crate::app_manifest::AppManifest;
use crate::approval::{ApprovalConsumeError, ApprovalStore};
use crate::capability::permissions::{decide_shell_policy, ShellPolicyDecision};
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRunParams {
    pub command: String,
    pub approval_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellRunOutcome {
    Completed(ShellRunResult),
    PendingApproval(PendingApprovalReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalReceipt {
    pub approval_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStage {
    pub command: String,
}

pub fn parse_pipeline(input: &str) -> Result<Vec<ParsedStage>, CapabilityError> {
    let stages = split_top_level_pipelines(input)?;
    if stages.is_empty() {
        return Err(invalid_shell_syntax("shell command is empty"));
    }

    stages
        .into_iter()
        .map(|stage| {
            let words = split_stage_words(&stage)?;
            let command = words
                .into_iter()
                .next()
                .ok_or_else(|| invalid_shell_syntax("shell stage is empty"))?;
            if command.contains('=') {
                return Err(invalid_shell_syntax(
                    "leading environment assignments are not supported",
                ));
            }
            Ok(ParsedStage { command })
        })
        .collect()
}

pub async fn handle_shell_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
    approvals: &mut ApprovalStore,
) -> Result<Vec<u8>, CapabilityError> {
    if method != "run" {
        return Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown shell method: {method}"),
        });
    }

    let params: ShellRunParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid params: {e}"),
    })?;
    let stages = parse_pipeline(&params.command)?;
    let commands: Vec<String> = stages.into_iter().map(|stage| stage.command).collect();

    if let Some(approval_id) = &params.approval_id {
        approvals
            .consume_approved(
                approval_id,
                manifest.app_id.as_str(),
                "shell",
                &params.command,
            )
            .map_err(map_approval_consume_error)?;
        return encode_outcome(run_command(&params.command)?);
    }

    match decide_shell_policy(manifest, &commands) {
        ShellPolicyDecision::Allow => encode_outcome(run_command(&params.command)?),
        ShellPolicyDecision::Ask => {
            let approval_id = approvals.create(
                manifest.app_id.as_str().to_string(),
                "shell".to_string(),
                params.command,
                commands,
            );
            encode_outcome(ShellRunOutcome::PendingApproval(PendingApprovalReceipt {
                approval_id,
            }))
        }
        ShellPolicyDecision::Deny => Err(CapabilityError {
            code: "permission_denied".to_string(),
            message: format!(
                "shell command is not allowed for app {:?}",
                manifest.app_id.as_str()
            ),
        }),
    }
}

fn split_top_level_pipelines(input: &str) -> Result<Vec<String>, CapabilityError> {
    let mut stages = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if ch == '\\' && !in_single {
            current.push(ch);
            if let Some(next) = chars.next() {
                current.push(next);
            }
            continue;
        }

        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '|' if !in_single && !in_double => {
                if matches!(chars.peek(), Some('|')) {
                    return Err(invalid_shell_syntax("operator || is not supported"));
                }
                let stage = current.trim();
                if stage.is_empty() {
                    return Err(invalid_shell_syntax("shell stage is empty"));
                }
                stages.push(stage.to_string());
                current.clear();
            }
            '&' if !in_single && !in_double => {
                return Err(invalid_shell_syntax("operator & is not supported"));
            }
            ';' | '<' | '>' | '`' | '(' | ')' | '{' | '}' if !in_single && !in_double => {
                return Err(invalid_shell_syntax("shell syntax is not supported"));
            }
            '$' if !in_single && !in_double && matches!(chars.peek(), Some('(')) => {
                return Err(invalid_shell_syntax(
                    "command substitution is not supported",
                ));
            }
            _ => current.push(ch),
        }
    }

    if in_single || in_double {
        return Err(invalid_shell_syntax("unterminated quoted string"));
    }

    let tail = current.trim();
    if tail.is_empty() {
        return Err(invalid_shell_syntax("shell command is empty"));
    }
    stages.push(tail.to_string());
    Ok(stages)
}

fn split_stage_words(stage: &str) -> Result<Vec<String>, CapabilityError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = stage.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if ch == '\\' && !in_single {
            if let Some(next) = chars.next() {
                current.push(next);
            }
            continue;
        }

        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ch if ch.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if in_single || in_double {
        return Err(invalid_shell_syntax("unterminated quoted string"));
    }

    if !current.is_empty() {
        words.push(current);
    }

    Ok(words)
}

fn invalid_shell_syntax(message: &str) -> CapabilityError {
    CapabilityError {
        code: "invalid_params".to_string(),
        message: message.to_string(),
    }
}

fn map_approval_consume_error(error: ApprovalConsumeError) -> CapabilityError {
    match error {
        ApprovalConsumeError::Mismatch => CapabilityError {
            code: "approval_mismatch".to_string(),
            message: "approval_id does not match this command".to_string(),
        },
        ApprovalConsumeError::NotFound
        | ApprovalConsumeError::Rejected
        | ApprovalConsumeError::Expired => CapabilityError {
            code: "approval_denied".to_string(),
            message: "approval was rejected or is unavailable".to_string(),
        },
        ApprovalConsumeError::Pending => CapabilityError {
            code: "approval_denied".to_string(),
            message: "approval is still pending".to_string(),
        },
    }
}

fn run_command(command: &str) -> Result<ShellRunOutcome, CapabilityError> {
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| CapabilityError {
            code: "io_error".to_string(),
            message: e.to_string(),
        })?;

    Ok(ShellRunOutcome::Completed(ShellRunResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
    }))
}

fn encode_outcome(outcome: ShellRunOutcome) -> Result<Vec<u8>, CapabilityError> {
    postcard::to_stdvec(&outcome).map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("encode result: {e}"),
    })
}
