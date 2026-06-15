use crate::app_manifest::AppRegistry;
use crate::ipc_server::CoreIpcServer;
use crate::worker_dispatch::{DispatchResult, WorkerManager};
use crate::worker_registry::WorkerRegistry;
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingResponse,
    CoreStatusResponse, CORE_CONTROL_SCHEMA,
};
use kunkka_worker_sdk::{AppId, WORKER_PROTOCOL_SCHEMA};
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};

const IDLE_REAP_INTERVAL: Duration = Duration::from_millis(100);

pub struct CoreRuntime {
    server: CoreIpcServer,
    worker_manager: WorkerManager,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;
        let app_registry = AppRegistry::load(paths)?;

        Ok(Self {
            server,
            worker_manager: WorkerManager::with_app_registry(
                app_registry,
                paths.socket_path.clone(),
            ),
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        self.worker_manager.registry()
    }

    pub fn worker_manager(&self) -> &WorkerManager {
        &self.worker_manager
    }

    pub fn reap_idle_workers(&mut self) {
        self.worker_manager.reap_idle_workers();
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
        run_connection(&self.server, &mut self.worker_manager, connection).await
    }

    pub async fn run(mut self) -> Result<()> {
        let mut reap_interval = interval(IDLE_REAP_INTERVAL);
        reap_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                accepted = self.server.accept_one() => {
                    let connection = accepted?;
                    run_connection(&self.server, &mut self.worker_manager, connection).await?;
                }
                _ = reap_interval.tick() => {
                    self.worker_manager.reap_idle_workers();
                }
            }
        }
    }
}

async fn run_connection(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    mut connection: IpcConnection,
) -> Result<()> {
    let Some(first_frame) = connection.recv_frame().await? else {
        return Ok(());
    };

    match frame_schema(&first_frame) {
        Some(WORKER_PROTOCOL_SCHEMA) => {
            worker_manager
                .handle_registration_connection(first_frame, connection, None, 300_000)
                .await
        }
        Some(CORE_CONTROL_SCHEMA) => {
            run_control_connection(server, worker_manager, connection, first_frame).await
        }
        Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
            "unknown payload schema: {schema}"
        ))),
        None => Err(CoreError::InvalidCoreFrame(
            "missing payload schema".to_string(),
        )),
    }
}

async fn run_control_connection(
    server: &CoreIpcServer,
    worker_manager: &mut WorkerManager,
    mut connection: IpcConnection,
    first_frame: Frame,
) -> Result<()> {
    let response = handle_control_frame(server, worker_manager.registry(), first_frame)?;
    connection.send_frame(&response).await?;
    let mut reap_interval = interval(IDLE_REAP_INTERVAL);
    reap_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            frame = connection.recv_frame() => {
                let Some(frame) = frame? else {
                    return Ok(());
                };
                let response = handle_control_frame(server, worker_manager.registry(), frame)?;
                connection.send_frame(&response).await?;
            }
            _ = reap_interval.tick() => {
                worker_manager.reap_idle_workers();
            }
        }
    }
}

fn handle_control_frame(
    server: &CoreIpcServer,
    registry: &WorkerRegistry,
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
