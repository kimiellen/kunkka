use crate::app_manifest::{AppManifest, AppRegistry};
use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::WorkerRegistry;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_worker_sdk::{
    decode_worker_message, encode_worker_message, AppId, DispatchWorkerRequest,
    DispatchWorkerResponse, RegisterWorkerRequest, RegisterWorkerResponse, WorkerId,
    WorkerProtocolMessage,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Ok(Payload),
    AppError { code: String, message: String },
}

pub struct WorkerManager {
    registry: WorkerRegistry,
    app_registry: AppRegistry,
    socket_path: PathBuf,
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
            app_registry: AppRegistry::default(),
            socket_path: PathBuf::new(),
            active_workers: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    pub fn with_app_registry(app_registry: AppRegistry, socket_path: PathBuf) -> Self {
        Self {
            registry: WorkerRegistry::new(),
            app_registry,
            socket_path,
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

    pub async fn dispatch_with_start(
        &mut self,
        server: &CoreIpcServer,
        app_id: AppId,
        method: String,
        payload: Payload,
    ) -> Result<DispatchResult> {
        if !self.is_active(&app_id) {
            self.start_and_wait_for_registration(server, &app_id)
                .await?;
        }

        self.dispatch(app_id, method, payload).await
    }

    async fn start_and_wait_for_registration(
        &mut self,
        server: &CoreIpcServer,
        app_id: &AppId,
    ) -> Result<()> {
        let manifest = self
            .app_registry
            .get_app(app_id)
            .cloned()
            .ok_or_else(|| CoreError::AppNotFound(app_id.as_str().to_string()))?;

        let mut child = spawn_worker(&manifest, &self.socket_path)?;
        let startup_timeout = Duration::from_millis(manifest.startup_timeout_ms);
        let registration = timeout(startup_timeout, async {
            let mut candidates = JoinSet::new();

            loop {
                tokio::select! {
                    accepted = server.accept_one() => {
                        let connection = accepted?;
                        candidates.spawn(read_registration_candidate(connection));
                    }
                    Some(candidate) = candidates.join_next(), if !candidates.is_empty() => {
                        let Ok(Some((frame, connection))) = candidate else {
                            continue;
                        };

                        if registration_matches_expected_app(&frame, app_id) {
                            candidates.abort_all();
                            return Ok::<_, CoreError>((frame, connection));
                        }
                    }
                }
            }
        })
        .await;

        match registration {
            Ok(Ok((frame, connection))) => {
                self.handle_registration_connection_with_expected_app(
                    frame,
                    connection,
                    Some(child),
                    manifest.idle_timeout_ms,
                    Some(app_id),
                )
                .await
            }
            Ok(Err(err)) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(err)
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(CoreError::WorkerStartTimeout(format!(
                    "worker for app {} did not register within {} ms",
                    app_id.as_str(),
                    manifest.startup_timeout_ms
                )))
            }
        }
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

    pub async fn handle_registration_connection(
        &mut self,
        frame: Frame,
        connection: IpcConnection,
        child: Option<Child>,
        idle_timeout_ms: u64,
    ) -> Result<()> {
        self.handle_registration_connection_with_expected_app(
            frame,
            connection,
            child,
            idle_timeout_ms,
            None,
        )
        .await
    }

    async fn handle_registration_connection_with_expected_app(
        &mut self,
        frame: Frame,
        mut connection: IpcConnection,
        mut child: Option<Child>,
        idle_timeout_ms: u64,
        expected_app_id: Option<&AppId>,
    ) -> Result<()> {
        let result = async {
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

            if let Some(expected_app_id) = expected_app_id {
                if request.app_id != *expected_app_id
                    || request.worker_id.as_str() != expected_app_id.as_str()
                {
                    return Err(CoreError::UnexpectedWorkerResponse(format!(
                        "worker registration mismatch for app {}: got app {} worker {}",
                        expected_app_id.as_str(),
                        request.app_id.as_str(),
                        request.worker_id.as_str()
                    )));
                }
            }

            let response = RegisterWorkerResponse {
                worker_id: request.worker_id.clone(),
                accepted: true,
                message: None,
            };
            let response_payload =
                encode_worker_message(&WorkerProtocolMessage::RegisterWorkerAccepted(response))?;
            let response_frame = Frame::Response {
                request_id,
                session_id,
                source: target_or_core(target),
                target: source,
                payload: response_payload,
                metadata: FrameMetadata::new(),
            };

            connection.send_frame(&response_frame).await?;
            Ok::<_, CoreError>((request, connection))
        }
        .await;

        match result {
            Ok((request, connection)) => {
                self.insert_active_worker(request, connection, child.take(), idle_timeout_ms);
                Ok(())
            }
            Err(err) => {
                if let Some(child) = child.as_mut() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(err)
            }
        }
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

fn spawn_worker(manifest: &AppManifest, socket_path: &Path) -> Result<Child> {
    let mut command = Command::new(&manifest.worker.program);
    command.args(&manifest.worker.args);
    for (key, value) in &manifest.worker.env {
        command.env(key, value);
    }
    command.env("KUNKKA_CORE_SOCKET", socket_path.as_os_str());
    command.env("KUNKKA_APP_ID", manifest.app_id.as_str());
    command.env("KUNKKA_WORKER_ID", manifest.app_id.as_str());
    if let Some(cwd) = &manifest.worker.cwd {
        command.current_dir(cwd);
    }

    command.spawn().map_err(|err| {
        CoreError::WorkerStartFailed(format!(
            "failed to start worker for app {}: {err}",
            manifest.app_id.as_str()
        ))
    })
}

async fn read_registration_candidate(
    mut connection: IpcConnection,
) -> Option<(Frame, IpcConnection)> {
    let frame = connection.recv_frame().await.ok()??;
    Some((frame, connection))
}

fn registration_matches_expected_app(frame: &Frame, expected_app_id: &AppId) -> bool {
    let Frame::Request { payload, .. } = frame else {
        return false;
    };

    let Ok(WorkerProtocolMessage::RegisterWorker(request)) = decode_worker_message(payload) else {
        return false;
    };

    request.app_id == *expected_app_id && request.worker_id.as_str() == expected_app_id.as_str()
}

fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
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
