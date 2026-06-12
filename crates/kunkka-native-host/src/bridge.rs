use crate::native_protocol::{
    error_response, success_response, NativeCommand, NativeRequest, NativeResponse, NativeResult,
};
use crate::{NativeHostError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse, CoreStatusRequest,
};
use std::path::{Path, PathBuf};

pub fn core_message_for_command(command: &NativeCommand) -> CoreControlMessage {
    match command {
        NativeCommand::Ping => CoreControlMessage::Ping(CorePingRequest),
        NativeCommand::Status => CoreControlMessage::Status(CoreStatusRequest),
    }
}

pub fn native_result_for_core_response(
    command: &NativeCommand,
    message: CoreControlMessage,
) -> Result<NativeResult> {
    match (command, message) {
        (NativeCommand::Ping, CoreControlMessage::Pong(CorePingResponse)) => Ok(NativeResult::Pong),
        (NativeCommand::Status, CoreControlMessage::StatusResult(status)) => {
            Ok(NativeResult::Status {
                worker_count: status.worker_count,
                socket_path: status.socket_path,
                runtime_ready: status.runtime_ready,
            })
        }
        (command, message) => Err(NativeHostError::UnexpectedCoreResponse(format!(
            "unexpected core response for {command:?}: {message:?}"
        ))),
    }
}

pub struct NativeHostSession {
    socket_path: PathBuf,
    connection: Option<IpcConnection>,
    source: EndpointId,
    target: EndpointId,
    session_id: SessionId,
    next_request_id: u128,
}

impl NativeHostSession {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            connection: None,
            source: EndpointId::new("native-host"),
            target: EndpointId::new("core"),
            session_id: SessionId(1),
            next_request_id: 1,
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn handle_request(&mut self, request: NativeRequest) -> NativeResponse {
        let id = request.id.clone();
        let command = request.command.clone();
        let core_message = core_message_for_command(&command);

        match self.send_core_control(core_message).await {
            Ok(response) => match native_result_for_core_response(&command, response) {
                Ok(result) => success_response(id, result),
                Err(err) => error_response(Some(id), err.code(), err.to_string()),
            },
            Err(err) => error_response(Some(id), err.code(), err.to_string()),
        }
    }

    async fn send_core_control(
        &mut self,
        message: CoreControlMessage,
    ) -> Result<CoreControlMessage> {
        self.ensure_connection().await?;
        let result = self.send_core_control_on_cached_connection(message).await;

        if result.is_err() {
            self.connection = None;
        }

        result
    }

    async fn ensure_connection(&mut self) -> Result<()> {
        if self.connection.is_some() {
            return Ok(());
        }

        let connection = IpcConnection::connect(&self.socket_path)
            .await
            .map_err(|err| NativeHostError::CoreUnavailable(err.to_string()))?;
        self.connection = Some(connection);
        Ok(())
    }

    async fn send_core_control_on_cached_connection(
        &mut self,
        message: CoreControlMessage,
    ) -> Result<CoreControlMessage> {
        let request_id = self.next_request_id();
        let payload = encode_control_message(&message)
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?;
        let frame = Frame::Request {
            request_id,
            session_id: self.session_id,
            source: self.source.clone(),
            target: self.target.clone(),
            payload,
            metadata: FrameMetadata::new(),
        };

        let connection = self.connection.as_mut().ok_or_else(|| {
            NativeHostError::CoreUnavailable("core connection missing".to_string())
        })?;

        connection
            .send_frame(&frame)
            .await
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?;
        let response = connection
            .recv_frame()
            .await
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?
            .ok_or_else(|| NativeHostError::CoreIpc("core closed connection".to_string()))?;

        let Frame::Response {
            request_id: response_request_id,
            payload,
            ..
        } = response
        else {
            return Err(NativeHostError::UnexpectedCoreResponse(
                "expected response frame".to_string(),
            ));
        };

        if response_request_id != request_id {
            return Err(NativeHostError::UnexpectedCoreResponse(format!(
                "response request_id mismatch: expected {}, got {}",
                request_id.0, response_request_id.0
            )));
        }

        decode_control_message(&payload).map_err(|err| NativeHostError::CoreIpc(err.to_string()))
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}
