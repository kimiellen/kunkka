use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::{handle_worker_registration_frame, WorkerRegistry};
use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingResponse,
    CoreStatusResponse, CORE_CONTROL_SCHEMA,
};
use kunkka_worker_sdk::WORKER_PROTOCOL_SCHEMA;

pub struct CoreRuntime {
    server: CoreIpcServer,
    registry: WorkerRegistry,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;

        Ok(Self {
            server,
            registry: WorkerRegistry::new(),
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }

    fn handle_frame(&mut self, frame: Frame) -> Result<Frame> {
        match frame_schema(&frame) {
            Some(WORKER_PROTOCOL_SCHEMA) => {
                handle_worker_registration_frame(&mut self.registry, frame)
            }
            Some(CORE_CONTROL_SCHEMA) => self.handle_control_frame(frame),
            Some(schema) => Err(CoreError::InvalidCoreFrame(format!(
                "unknown payload schema: {schema}"
            ))),
            None => Err(CoreError::InvalidCoreFrame(
                "missing payload schema".to_string(),
            )),
        }
    }

    pub async fn run_once(&mut self) -> Result<()> {
        let mut connection = self.server.accept_one().await?;

        while let Some(frame) = connection.recv_frame().await? {
            let response = self.handle_frame(frame)?;
            connection.send_frame(&response).await?;
        }

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            self.run_once().await?;
        }
    }

    fn handle_control_frame(&self, frame: Frame) -> Result<Frame> {
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
                worker_count: self.registry.len() as u64,
                socket_path: self.server.socket_path().to_string_lossy().into_owned(),
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
