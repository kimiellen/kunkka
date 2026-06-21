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
    LlmPresets {
        presets: Vec<LlmPresetResult>,
    },
    LlmProviders {
        providers: Vec<LlmProviderResult>,
    },
    LlmRoles {
        roles: Vec<LlmRoleResult>,
    },
    LlmProviderTest {
        name: String,
        success: bool,
        latency_ms: Option<u64>,
        error: Option<String>,
    },
    LlmUsageSummary {
        total_requests: u64,
        total_prompt_tokens: u64,
        total_completion_tokens: u64,
        total_tokens: u64,
    },
    LlmUsageRecords {
        records: Vec<LlmUsageRecordResult>,
    },
    LlmDefaultRole {
        role_name: Option<String>,
    },
    LlmSuccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmPresetResult {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProviderResult {
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub available_models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRoleResult {
    pub name: String,
    pub description: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProviderTestResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmUsageRecordResult {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub role: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
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
