use kunkka_protocol::core_control::PendingApproval;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliOutput {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CliResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CliErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliResult {
    Pong,
    Status {
        worker_count: u64,
        socket_path: String,
        runtime_ready: bool,
    },
    Dispatch {
        payload: serde_json::Value,
    },
    DispatchError {
        code: String,
        message: String,
    },
    PendingApprovals {
        approvals: Vec<PendingApproval>,
    },
    ApprovalDecision,
    ShellResult {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    ApprovalPrompt {
        approval_id: String,
        app_id: String,
        capability: String,
        summary: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliErrorBody {
    pub code: String,
    pub message: String,
}

impl CliOutput {
    pub fn success(result: CliResult) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(CliErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    pub fn is_success(&self) -> bool {
        self.ok
    }
}
