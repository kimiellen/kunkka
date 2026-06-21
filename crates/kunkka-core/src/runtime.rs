use crate::app_manifest::AppRegistry;
use crate::approval::ApprovalStore;
use crate::capability::{
    decode_capability_request, encode_capability_response, handle_capability_request,
    llm::LlmState, sqlite::SqliteConnectionStore, CAPABILITY_SCHEMA,
};
use crate::database::CoreDatabase;
use crate::ipc_server::CoreIpcServer;
use crate::theme::{ThemeFlavor as CoreThemeFlavor, ThemeManager};
use crate::worker_dispatch::{DispatchResult, WorkerManager};
use crate::worker_registry::WorkerRegistry;
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use futures_util::StreamExt;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, StreamId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApprovalDecisionResponse,
    CoreControlMessage, CoreGetThemeResponse, CoreListApprovalsResponse, CorePingResponse,
    CoreSetThemeResponse, CoreStatusResponse, ThemeChangedEvent,
    ThemeFlavor as ProtocolThemeFlavor, CORE_CONTROL_SCHEMA,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse, FRONTEND_DISPATCH_SCHEMA,
};
use kunkka_worker_sdk::{AppId, WORKER_PROTOCOL_SCHEMA};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, info, warn};

const IDLE_REAP_INTERVAL: Duration = Duration::from_millis(100);
const THEME_BROADCAST_CAPACITY: usize = 16;

pub struct CoreRuntime {
    server: CoreIpcServer,
    worker_manager: WorkerManager,
    approvals: ApprovalStore,
    _database: CoreDatabase,
    sqlite_connections: SqliteConnectionStore,
    data_dir: std::path::PathBuf,
    llm_state: LlmState,
    theme_manager: ThemeManager,
    theme_broadcast: broadcast::Sender<ProtocolThemeFlavor>,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        info!("initializing core runtime");
        let database = CoreDatabase::connect(paths).await?;
        let server = CoreIpcServer::bind(paths).await?;
        let app_registry = AppRegistry::load(paths)?;

        let llm_state = LlmState::new(paths);
        llm_state
            .initialize()
            .await
            .map_err(|e| CoreError::Config(format!("Failed to initialize LLM state: {:?}", e)))?;

        let theme_manager = ThemeManager::load_from_dir(&paths.config_dir)
            .map_err(|e| CoreError::Config(format!("Failed to load theme config: {}", e)))?;

        let (theme_broadcast, _) = broadcast::channel(THEME_BROADCAST_CAPACITY);

        Ok(Self {
            server,
            worker_manager: WorkerManager::with_app_registry(
                app_registry,
                paths.socket_path.clone(),
            ),
            approvals: ApprovalStore::new(),
            _database: database,
            sqlite_connections: SqliteConnectionStore::new(),
            data_dir: paths.data_dir.clone(),
            llm_state,
            theme_manager,
            theme_broadcast,
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        self.worker_manager.registry()
    }

    pub fn worker_manager(&self) -> &WorkerManager {
        &self.worker_manager
    }

    pub fn database(&self) -> &CoreDatabase {
        &self._database
    }

    pub fn theme_manager(&self) -> &ThemeManager {
        &self.theme_manager
    }

    pub fn theme_manager_mut(&mut self) -> &mut ThemeManager {
        &mut self.theme_manager
    }

    pub fn reap_idle_workers(&mut self) {
        self.worker_manager.reap_idle_workers();
    }

    pub fn expire_pending_approval_for_test(&mut self, approval_id: &str) {
        self.approvals.expire(approval_id);
    }

    pub async fn dispatch(
        &mut self,
        app_id: AppId,
        method: String,
        payload: Payload,
    ) -> Result<DispatchResult> {
        self.worker_manager
            .dispatch_with_start(&self.server, app_id, method, payload)
            .await
    }

    pub async fn run_once(&mut self) -> Result<()> {
        let connection = self.server.accept_one().await?;
        run_connection(
            &self.server,
            &mut self.worker_manager,
            &mut self.approvals,
            &self._database,
            &mut self.sqlite_connections,
            &self.data_dir,
            &self.llm_state,
            &mut self.theme_manager,
            &self.theme_broadcast,
            connection,
        )
        .await
    }

    pub async fn run(mut self) -> Result<()> {
        info!("core runtime loop started");
        let mut reap_interval = interval(IDLE_REAP_INTERVAL);
        reap_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                accepted = self.server.accept_one() => {
                    let connection = accepted?;
                    run_connection(&self.server, &mut self.worker_manager, &mut self.approvals, &self._database, &mut self.sqlite_connections, &self.data_dir, &self.llm_state, &mut self.theme_manager, &self.theme_broadcast, connection).await?;
                }
                _ = reap_interval.tick() => {
                    self.worker_manager.reap_idle_workers();
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_connection(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    approvals: &mut ApprovalStore,
    database: &CoreDatabase,
    sqlite_connections: &mut SqliteConnectionStore,
    data_dir: &std::path::Path,
    llm_state: &LlmState,
    theme_manager: &mut ThemeManager,
    theme_broadcast: &broadcast::Sender<ProtocolThemeFlavor>,
    mut connection: IpcConnection,
) -> Result<()> {
    let Some(first_frame) = connection.recv_frame().await? else {
        return Ok(());
    };

    match frame_schema(&first_frame) {
        Some(WORKER_PROTOCOL_SCHEMA) => {
            debug!("routing to worker registration");
            worker_manager
                .handle_registration_connection(first_frame, connection, None, 300_000)
                .await
        }
        Some(CORE_CONTROL_SCHEMA | FRONTEND_DISPATCH_SCHEMA) => {
            let mut theme_rx = theme_broadcast.subscribe();
            run_frontend_connection(
                server,
                worker_manager,
                approvals,
                database,
                theme_manager,
                theme_broadcast,
                &mut theme_rx,
                connection,
                first_frame,
            )
            .await
        }
        Some(CAPABILITY_SCHEMA) => {
            handle_capability_connection(
                worker_manager,
                approvals,
                sqlite_connections,
                data_dir,
                llm_state,
                connection,
                first_frame,
            )
            .await
        }
        Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
            "unknown payload schema: {schema}"
        ))),
        None => Err(CoreError::InvalidCoreFrame(
            "missing payload schema".to_string(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_frontend_connection(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    approvals: &mut ApprovalStore,
    database: &CoreDatabase,
    theme_manager: &mut ThemeManager,
    theme_broadcast: &broadcast::Sender<ProtocolThemeFlavor>,
    theme_rx: &mut broadcast::Receiver<ProtocolThemeFlavor>,
    mut connection: IpcConnection,
    first_frame: Frame,
) -> Result<()> {
    let response = handle_frontend_frame(
        server,
        worker_manager,
        approvals,
        database,
        theme_manager,
        theme_broadcast,
        first_frame,
    )
    .await?;
    connection.send_frame(&response).await?;
    let mut reap_interval = interval(IDLE_REAP_INTERVAL);
    reap_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            frame = connection.recv_frame() => {
                let Some(frame) = frame? else {
                    return Ok(());
                };
                let response = handle_frontend_frame(server, worker_manager, approvals, database, theme_manager, theme_broadcast, frame).await?;
                connection.send_frame(&response).await?;
            }
            theme_result = theme_rx.recv() => {
                match theme_result {
                    Ok(flavor) => {
                        let event_payload = encode_control_message(
                            &CoreControlMessage::ThemeChanged(ThemeChangedEvent { flavor }),
                        )?;
                        let event_frame = Frame::Event {
                            session_id: kunkka_ipc::SessionId(0),
                            source: EndpointId::new("core"),
                            target: EndpointId::new("frontend"),
                            name: "theme_changed".to_string(),
                            payload: event_payload,
                            metadata: FrameMetadata::new(),
                        };
                        if connection.send_frame(&event_frame).await.is_err() {
                            return Ok(());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            _ = reap_interval.tick() => {
                worker_manager.reap_idle_workers();
            }
        }
    }
}

async fn handle_capability_connection(
    worker_manager: &WorkerManager,
    approvals: &mut ApprovalStore,
    sqlite_connections: &mut SqliteConnectionStore,
    data_dir: &std::path::Path,
    llm_state: &LlmState,
    mut connection: IpcConnection,
    frame: Frame,
) -> Result<()> {
    let Frame::Request {
        request_id,
        session_id,
        source,
        target,
        payload,
        ..
    } = frame
    else {
        return Err(CoreError::InvalidCoreFrame(
            "expected request frame".to_string(),
        ));
    };

    let request = decode_capability_request(&payload)?;

    debug!(
        app_id = %request.app_id,
        capability = %request.capability,
        method = %request.method,
        "capability request"
    );

    if request.capability == "llm"
        && crate::capability::llm::is_streaming_chat(&request.method, &request.params).map_err(
            |err| CoreError::InvalidCoreFrame(format!("llm stream decode: {}", err.message)),
        )?
    {
        return handle_llm_stream_connection(
            llm_state,
            request,
            request_id,
            session_id,
            source,
            target,
            &mut connection,
        )
        .await;
    }

    let response = handle_capability_request(
        worker_manager.app_registry(),
        approvals,
        request,
        Some(sqlite_connections),
        data_dir,
        Some(llm_state),
    )
    .await;
    let response_payload = encode_capability_response(&response)?;

    let response_frame = Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload: response_payload,
        metadata: FrameMetadata::new(),
    };

    connection.send_frame(&response_frame).await?;
    Ok(())
}

async fn handle_llm_stream_connection(
    llm_state: &LlmState,
    request: crate::capability::CapabilityRequest,
    request_id: kunkka_ipc::RequestId,
    session_id: kunkka_ipc::SessionId,
    source: EndpointId,
    target: EndpointId,
    connection: &mut IpcConnection,
) -> Result<()> {
    let chat_params =
        crate::capability::llm::decode_chat_params(&request.params).map_err(|err| {
            CoreError::InvalidCoreFrame(format!("llm stream decode: {}", err.message))
        })?;

    let mut stream = match crate::capability::llm::create_chat_stream(chat_params, llm_state).await
    {
        Ok(stream) => stream,
        Err(err) => {
            let response = crate::capability::CapabilityResponse { result: Err(err) };
            let payload = encode_capability_response(&response)?;
            connection
                .send_frame(&Frame::Response {
                    request_id,
                    session_id,
                    source: target_or_core(target),
                    target: source,
                    payload,
                    metadata: FrameMetadata::new(),
                })
                .await?;
            return Ok(());
        }
    };

    let stream_id = StreamId(request_id.0);

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                let usage = chunk.usage.map(|u| crate::capability::llm::LlmUsage {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                });

                for choice in chunk.choices {
                    let event = crate::capability::llm::LlmChatStreamChunk {
                        content_delta: choice.delta.content.unwrap_or_default(),
                        finish_reason: choice.finish_reason.as_ref().map(|r| format!("{:?}", r)),
                        usage: usage.clone(),
                    };

                    if event.content_delta.is_empty()
                        && event.finish_reason.is_none()
                        && event.usage.is_none()
                    {
                        continue;
                    }

                    let payload = Payload {
                        bytes: postcard::to_stdvec(&event).map_err(|e| {
                            CoreError::InvalidCoreFrame(format!("llm stream encode: {e}"))
                        })?,
                        content_type: Some(crate::capability::CAPABILITY_CONTENT_TYPE.to_string()),
                        schema: Some(crate::capability::CAPABILITY_SCHEMA.to_string()),
                        metadata: FrameMetadata::new(),
                    };

                    connection
                        .send_frame(&Frame::Stream {
                            stream_id,
                            request_id: Some(request_id),
                            session_id,
                            source: target_or_core(target.clone()),
                            target: source.clone(),
                            payload,
                            end: false,
                            metadata: FrameMetadata::new(),
                        })
                        .await?;
                }
            }
            Err(err) => {
                connection
                    .send_frame(&Frame::Error {
                        request_id: Some(request_id),
                        stream_id: Some(stream_id),
                        session_id: Some(session_id),
                        source: target_or_core(target.clone()),
                        target: source.clone(),
                        code: "llm_stream_error".to_string(),
                        message: err.to_string(),
                        metadata: FrameMetadata::new(),
                    })
                    .await?;
                return Ok(());
            }
        }
    }

    connection
        .send_frame(&Frame::Stream {
            stream_id,
            request_id: Some(request_id),
            session_id,
            source: target_or_core(target),
            target: source,
            payload: Payload {
                bytes: Vec::new(),
                content_type: Some(crate::capability::CAPABILITY_CONTENT_TYPE.to_string()),
                schema: Some(crate::capability::CAPABILITY_SCHEMA.to_string()),
                metadata: FrameMetadata::new(),
            },
            end: true,
            metadata: FrameMetadata::new(),
        })
        .await?;

    Ok(())
}

async fn handle_frontend_frame(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    approvals: &mut ApprovalStore,
    database: &CoreDatabase,
    theme_manager: &mut ThemeManager,
    theme_broadcast: &broadcast::Sender<ProtocolThemeFlavor>,
    frame: Frame,
) -> Result<Frame> {
    match frame_schema(&frame) {
        Some(CORE_CONTROL_SCHEMA) => handle_control_frame(
            server,
            worker_manager.registry(),
            approvals,
            theme_manager,
            theme_broadcast,
            frame,
        ),
        Some(FRONTEND_DISPATCH_SCHEMA) => {
            handle_frontend_dispatch_frame(server, worker_manager, database, frame).await
        }
        Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
            "unknown payload schema: {schema}"
        ))),
        None => Err(CoreError::InvalidCoreFrame(
            "missing payload schema".to_string(),
        )),
    }
}

async fn handle_frontend_dispatch_frame(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    database: &CoreDatabase,
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
        return Err(CoreError::InvalidCoreFrame(
            "expected request frame".to_string(),
        ));
    };

    let response = match decode_frontend_dispatch_message(&payload)? {
        FrontendDispatchMessage::Dispatch(request) => {
            handle_frontend_dispatch_request(server, worker_manager, database, request).await?
        }
        _ => {
            return Err(CoreError::InvalidCoreFrame(
                "expected frontend dispatch request".to_string(),
            ));
        }
    };

    let payload =
        encode_frontend_dispatch_message(&FrontendDispatchMessage::DispatchResult(response))?;

    Ok(Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload,
        metadata: FrameMetadata::new(),
    })
}

async fn handle_frontend_dispatch_request(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    database: &CoreDatabase,
    request: FrontendDispatchRequest,
) -> Result<FrontendDispatchResponse> {
    debug!(
        app_id = %request.app_id,
        method = %request.method,
        "frontend dispatch request"
    );

    if request.app_id.is_empty() {
        return Ok(platform_error(
            "invalid_request",
            "dispatch app_id is empty",
        ));
    }
    if request.method.is_empty() {
        return Ok(platform_error(
            "invalid_request",
            "dispatch method is empty",
        ));
    }

    let Some(manifest) = worker_manager.app_registry().get(&request.app_id) else {
        if database
            .record_frontend_dispatch_audit(
                &request.app_id,
                &request.method,
                "deny",
                "app_not_found",
            )
            .await
            .is_err()
        {
            return Ok(platform_error("core_error", "audit write failed"));
        }
        return Ok(platform_error(
            "app_not_found",
            format!("app not found: {}", request.app_id),
        ));
    };

    match crate::permissions::decide_frontend_dispatch(manifest, &request.method) {
        crate::permissions::PermissionDecision::Deny { code, message } => {
            if database
                .record_frontend_dispatch_audit(&request.app_id, &request.method, "deny", code)
                .await
                .is_err()
            {
                return Ok(platform_error("core_error", "audit write failed"));
            }
            return Ok(platform_error(code, message));
        }
        crate::permissions::PermissionDecision::Allow => {}
    }

    if database
        .record_frontend_dispatch_audit(&request.app_id, &request.method, "allow", "allowed")
        .await
        .is_err()
    {
        return Ok(platform_error("core_error", "audit write failed"));
    }

    match worker_manager
        .dispatch_with_start(
            server,
            AppId::new(request.app_id),
            request.method,
            request.payload,
        )
        .await
    {
        Ok(DispatchResult::Ok(payload)) => Ok(FrontendDispatchResponse::Ok(payload)),
        Ok(DispatchResult::AppError { code, message }) => {
            Ok(FrontendDispatchResponse::AppError { code, message })
        }
        Err(err) => Ok(platform_error(
            dispatch_platform_error_code(&err),
            err.to_string(),
        )),
    }
}

fn platform_error(code: impl Into<String>, message: impl Into<String>) -> FrontendDispatchResponse {
    FrontendDispatchResponse::PlatformError {
        code: code.into(),
        message: message.into(),
    }
}

fn dispatch_platform_error_code(error: &CoreError) -> &'static str {
    match error {
        CoreError::AppNotFound(_) => "app_not_found",
        CoreError::WorkerStartFailed(_) => "worker_start_failed",
        CoreError::WorkerStartTimeout(_) => "worker_start_timeout",
        CoreError::WorkerUnavailable(_) => "worker_unavailable",
        CoreError::DispatchIpcError(_) => "dispatch_ipc_error",
        CoreError::UnexpectedWorkerResponse(_) => "unexpected_worker_response",
        CoreError::InvalidCoreFrame(_) => "invalid_request",
        _ => "core_error",
    }
}

fn handle_control_frame(
    server: &CoreIpcServer,
    registry: &WorkerRegistry,
    approvals: &mut ApprovalStore,
    theme_manager: &mut ThemeManager,
    theme_broadcast: &broadcast::Sender<ProtocolThemeFlavor>,
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
        return Err(CoreError::InvalidCoreFrame(
            "expected request frame".to_string(),
        ));
    };

    let response_message = match decode_control_message(&payload)? {
        CoreControlMessage::Ping(_) => CoreControlMessage::Pong(CorePingResponse),
        CoreControlMessage::Status(_) => CoreControlMessage::StatusResult(CoreStatusResponse {
            worker_count: registry.len() as u64,
            socket_path: server.socket_path().to_string_lossy().into_owned(),
            runtime_ready: true,
        }),
        CoreControlMessage::ListPendingApprovals(_) => {
            CoreControlMessage::PendingApprovalsResult(CoreListApprovalsResponse {
                approvals: approvals.list_pending(),
            })
        }
        CoreControlMessage::ApprovePendingApproval(request) => {
            approvals.approve(&request.approval_id);
            CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse)
        }
        CoreControlMessage::RejectPendingApproval(request) => {
            approvals.reject(&request.approval_id);
            CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse)
        }
        CoreControlMessage::GetTheme(_) => {
            let flavor = to_protocol_flavor(theme_manager.active_flavor());
            CoreControlMessage::GetThemeResult(CoreGetThemeResponse { flavor })
        }
        CoreControlMessage::SetTheme(request) => {
            let new_flavor = to_core_flavor(request.flavor);
            if let Err(e) = theme_manager.switch_flavor(new_flavor) {
                warn!(error = %e, "failed to switch theme");
                return Err(CoreError::Config(format!("Failed to switch theme: {e}")));
            }
            info!(flavor = ?request.flavor, "theme switched");
            let _ = theme_broadcast.send(request.flavor);
            CoreControlMessage::SetThemeResult(CoreSetThemeResponse)
        }
        _ => {
            return Err(CoreError::InvalidCoreFrame(
                "expected core control request".to_string(),
            ));
        }
    };

    let payload = encode_control_message(&response_message)?;

    Ok(Frame::Response {
        request_id,
        session_id,
        source: target_or_core(target),
        target: source,
        payload,
        metadata: FrameMetadata::new(),
    })
}

fn frame_schema(frame: &Frame) -> Option<&str> {
    match frame {
        Frame::Request { payload, .. }
        | Frame::Response { payload, .. }
        | Frame::Event { payload, .. }
        | Frame::Stream { payload, .. } => payload.schema.as_deref(),
        Frame::Cancel { .. } | Frame::Heartbeat { .. } | Frame::Error { .. } => None,
    }
}

fn target_or_core(target: EndpointId) -> EndpointId {
    if target.as_str().is_empty() {
        EndpointId::new("core")
    } else {
        target
    }
}

fn to_protocol_flavor(flavor: CoreThemeFlavor) -> ProtocolThemeFlavor {
    match flavor {
        CoreThemeFlavor::Latte => ProtocolThemeFlavor::Latte,
        CoreThemeFlavor::Macchiato => ProtocolThemeFlavor::Macchiato,
    }
}

fn to_core_flavor(flavor: ProtocolThemeFlavor) -> CoreThemeFlavor {
    match flavor {
        ProtocolThemeFlavor::Latte => CoreThemeFlavor::Latte,
        ProtocolThemeFlavor::Macchiato => CoreThemeFlavor::Macchiato,
    }
}
