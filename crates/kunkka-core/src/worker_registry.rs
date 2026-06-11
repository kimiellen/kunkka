use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, RegisterWorkerRequest,
    RegisterWorkerResponse, WorkerCapability, WorkerId, WorkerProtocolMessage,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredWorker {
    pub worker_id: WorkerId,
    pub app_id: AppId,
    pub capabilities: Vec<WorkerCapability>,
}

#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: BTreeMap<WorkerId, RegisteredWorker>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, request: RegisterWorkerRequest) -> RegisterWorkerResponse {
        let worker_id = request.worker_id.clone();

        let registered = RegisteredWorker {
            worker_id: request.worker_id,
            app_id: request.app_id,
            capabilities: request.capabilities,
        };

        self.workers.insert(worker_id.clone(), registered);

        RegisterWorkerResponse {
            worker_id,
            accepted: true,
            message: None,
        }
    }

    pub fn get(&self, worker_id: &WorkerId) -> Option<&RegisteredWorker> {
        self.workers.get(worker_id)
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }
}

pub fn handle_worker_registration_frame(
    registry: &mut WorkerRegistry,
    frame: Frame,
) -> Result<Frame> {
    let Frame::Request {
        request_id,
        session_id,
        source,
        target,
        payload,
        ..
    } = frame
    else {
        return Err(CoreError::InvalidWorkerFrame(
            "expected request frame".to_string(),
        ));
    };

    let message = decode_worker_message(&payload)?;

    let WorkerProtocolMessage::RegisterWorker(request) = message else {
        return Err(CoreError::InvalidWorkerFrame(
            "expected worker registration request".to_string(),
        ));
    };

    let response = registry.register(request);
    let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorkerAccepted(response))?;

    Ok(Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload,
        metadata: FrameMetadata::new(),
    })
}

fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
    }
}
