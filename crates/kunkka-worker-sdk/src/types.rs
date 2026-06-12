use kunkka_ipc::Payload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkerId(String);

impl WorkerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AppId(String);

impl AppId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapability {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWorkerRequest {
    pub worker_id: WorkerId,
    pub app_id: AppId,
    pub capabilities: Vec<WorkerCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWorkerResponse {
    pub worker_id: WorkerId,
    pub accepted: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchWorkerRequest {
    pub app_id: AppId,
    pub method: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerAppError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchWorkerResponse {
    Ok(Payload),
    Err(WorkerAppError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerProtocolMessage {
    RegisterWorker(RegisterWorkerRequest),
    RegisterWorkerAccepted(RegisterWorkerResponse),
    DispatchWorker(DispatchWorkerRequest),
    DispatchWorkerResult(DispatchWorkerResponse),
}
