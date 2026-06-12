use crate::worker_registry::WorkerRegistry;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, RegisterWorkerRequest, WorkerId, WorkerProtocolMessage,
};
use std::collections::BTreeMap;
use std::process::Child;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Ok(Payload),
    AppError { code: String, message: String },
}

pub struct WorkerManager {
    registry: WorkerRegistry,
    active_workers: BTreeMap<AppId, ActiveWorker>,
    next_request_id: u128,
}

struct ActiveWorker {
    worker_id: WorkerId,
    connection: IpcConnection,
    child: Option<Child>,
    #[allow(dead_code)]
    last_used_at: Instant,
    #[allow(dead_code)]
    idle_timeout: Duration,
}

impl WorkerManager {
    pub fn new_empty() -> Self {
        Self {
            registry: WorkerRegistry::new(),
            active_workers: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }

    pub fn is_active(&self, app_id: &AppId) -> bool {
        self.active_workers.contains_key(app_id)
    }

    pub fn active_worker_count(&self) -> usize {
        self.active_workers.len()
    }

    pub fn register_active_for_test(
        &mut self,
        request: RegisterWorkerRequest,
        connection: IpcConnection,
        idle_timeout_ms: u64,
    ) {
        self.insert_active_worker(request, connection, None, idle_timeout_ms);
    }

    pub fn insert_active_worker(
        &mut self,
        request: RegisterWorkerRequest,
        connection: IpcConnection,
        child: Option<Child>,
        idle_timeout_ms: u64,
    ) {
        let app_id = request.app_id.clone();
        let worker_id = request.worker_id.clone();
        let previous_app_id = self
            .registry
            .get(&worker_id)
            .map(|registered| registered.app_id.clone());

        if previous_app_id
            .as_ref()
            .is_some_and(|old_app_id| old_app_id != &app_id)
        {
            if let Some(mut old_worker) = previous_app_id
                .as_ref()
                .and_then(|old_app_id| self.active_workers.remove(old_app_id))
            {
                old_worker.terminate();
            }
        }

        self.registry.register(request);

        if let Some(mut old_worker) = self.active_workers.remove(&app_id) {
            old_worker.terminate();
        }

        self.active_workers.insert(
            app_id,
            ActiveWorker {
                worker_id,
                connection,
                child,
                last_used_at: Instant::now(),
                idle_timeout: Duration::from_millis(idle_timeout_ms),
            },
        );
    }

    pub async fn dispatch(
        &mut self,
        app_id: AppId,
        method: String,
        payload: Payload,
    ) -> Result<DispatchResult> {
        if method.is_empty() {
            return Err(CoreError::UnexpectedWorkerResponse(
                "dispatch method is empty".to_string(),
            ));
        }

        let request_id = self.next_request_id();
        let active = self.active_workers.get_mut(&app_id).ok_or_else(|| {
            CoreError::WorkerUnavailable(format!("no active worker for app {}", app_id.as_str()))
        })?;

        let request = DispatchWorkerRequest {
            app_id: app_id.clone(),
            method,
            payload,
        };
        let frame_payload = encode_worker_message(&WorkerProtocolMessage::DispatchWorker(request))?;
        let frame = Frame::Request {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new(format!("worker:{}", active.worker_id.as_str())),
            payload: frame_payload,
            metadata: FrameMetadata::new(),
        };

        let result = send_dispatch_frame(active, frame, request_id).await;
        match result {
            Ok(result) => {
                active.last_used_at = Instant::now();
                Ok(result)
            }
            Err(err) => {
                if let Some(mut removed) = self.active_workers.remove(&app_id) {
                    removed.terminate();
                }
                self.registry.remove_by_app_id(&app_id);
                Err(err)
            }
        }
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}

impl ActiveWorker {
    fn terminate(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn send_dispatch_frame(
    active: &mut ActiveWorker,
    frame: Frame,
    request_id: RequestId,
) -> Result<DispatchResult> {
    active
        .connection
        .send_frame(&frame)
        .await
        .map_err(|err| CoreError::DispatchIpcError(err.to_string()))?;

    let response = active
        .connection
        .recv_frame()
        .await
        .map_err(|err| CoreError::DispatchIpcError(err.to_string()))?
        .ok_or_else(|| CoreError::DispatchIpcError("worker closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CoreError::UnexpectedWorkerResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CoreError::UnexpectedWorkerResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    let message = decode_worker_message(&payload).map_err(|err| {
        CoreError::UnexpectedWorkerResponse(format!("failed to decode worker response: {err}"))
    })?;

    match message {
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Ok(payload)) => {
            Ok(DispatchResult::Ok(payload))
        }
        WorkerProtocolMessage::DispatchWorkerResult(DispatchWorkerResponse::Err(err)) => {
            Ok(DispatchResult::AppError {
                code: err.code,
                message: err.message,
            })
        }
        other => Err(CoreError::UnexpectedWorkerResponse(format!(
            "expected DispatchWorkerResult, got {other:?}"
        ))),
    }
}
