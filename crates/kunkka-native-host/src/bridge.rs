use crate::native_protocol::{
    error_response, success_response, NativeCommand, NativePendingApproval, NativeRequest,
    NativeResponse, NativeResult,
};
use crate::{NativeHostError, Result};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreApprovalDecisionResponse,
    CoreApproveApprovalRequest, CoreControlMessage, CoreListApprovalsRequest, CorePingRequest,
    CorePingResponse, CoreRejectApprovalRequest, CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use std::path::{Path, PathBuf};

const JSON_CONTENT_TYPE: &str = "application/json";

pub fn core_message_for_command(command: &NativeCommand) -> Result<CoreControlMessage> {
    match command {
        NativeCommand::Ping => Ok(CoreControlMessage::Ping(CorePingRequest)),
        NativeCommand::Status => Ok(CoreControlMessage::Status(CoreStatusRequest)),
        NativeCommand::Dispatch { .. } => Err(NativeHostError::InvalidRequest(
            "expected control command".to_string(),
        )),
        NativeCommand::ApprovalsList => Ok(CoreControlMessage::ListPendingApprovals(
            CoreListApprovalsRequest,
        )),
        NativeCommand::ApprovalApprove { approval_id } => Ok(
            CoreControlMessage::ApprovePendingApproval(CoreApproveApprovalRequest {
                approval_id: approval_id.clone(),
            }),
        ),
        NativeCommand::ApprovalReject { approval_id } => Ok(
            CoreControlMessage::RejectPendingApproval(CoreRejectApprovalRequest {
                approval_id: approval_id.clone(),
            }),
        ),
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
        (NativeCommand::ApprovalsList, CoreControlMessage::PendingApprovalsResult(response)) => {
            Ok(NativeResult::PendingApprovals {
                approvals: response
                    .approvals
                    .into_iter()
                    .map(|a| NativePendingApproval {
                        approval_id: a.approval_id,
                        app_id: a.app_id,
                        capability: a.capability,
                        summary: a.summary,
                    })
                    .collect(),
            })
        }
        (
            NativeCommand::ApprovalApprove { .. },
            CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse),
        ) => Ok(NativeResult::ApprovalDecision),
        (
            NativeCommand::ApprovalReject { .. },
            CoreControlMessage::ApprovalDecisionResult(CoreApprovalDecisionResponse),
        ) => Ok(NativeResult::ApprovalDecision),
        (command, message) => Err(NativeHostError::UnexpectedCoreResponse(format!(
            "unexpected core response for {command:?}: {message:?}"
        ))),
    }
}

pub fn frontend_dispatch_request_for_command(
    command: &NativeCommand,
) -> Result<FrontendDispatchRequest> {
    let NativeCommand::Dispatch {
        app_id,
        method,
        payload,
    } = command
    else {
        return Err(NativeHostError::InvalidRequest(
            "expected dispatch command".to_string(),
        ));
    };

    Ok(FrontendDispatchRequest {
        app_id: app_id.clone(),
        method: method.clone(),
        payload: json_payload_for_native_value(payload)?,
    })
}

fn json_payload_for_native_value(value: &serde_json::Value) -> Result<Payload> {
    let bytes = serde_json::to_vec(value)?;

    Ok(Payload {
        bytes,
        content_type: Some(JSON_CONTENT_TYPE.to_string()),
        schema: None,
        metadata: FrameMetadata::new(),
    })
}

pub fn native_result_for_frontend_dispatch_response(
    response: FrontendDispatchResponse,
) -> Result<NativeResult> {
    match response {
        FrontendDispatchResponse::Ok(payload) => Ok(NativeResult::Dispatch {
            payload: native_value_for_json_payload(&payload)?,
        }),
        FrontendDispatchResponse::AppError { code, message } => {
            Ok(NativeResult::DispatchError { code, message })
        }
        FrontendDispatchResponse::PlatformError { code, message } => {
            Err(NativeHostError::CorePlatform { code, message })
        }
    }
}

fn native_value_for_json_payload(payload: &Payload) -> Result<serde_json::Value> {
    if payload.content_type.as_deref() != Some(JSON_CONTENT_TYPE) {
        return Err(NativeHostError::UnexpectedCoreResponse(format!(
            "expected JSON dispatch payload, got {:?}",
            payload.content_type
        )));
    }

    serde_json::from_slice(&payload.bytes).map_err(NativeHostError::from)
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

        match request.command.clone() {
            NativeCommand::Ping
            | NativeCommand::Status
            | NativeCommand::ApprovalsList
            | NativeCommand::ApprovalApprove { .. }
            | NativeCommand::ApprovalReject { .. } => {
                let command = request.command.clone();
                let core_message = match core_message_for_command(&command) {
                    Ok(message) => message,
                    Err(err) => return error_response(Some(id), err.code(), err.to_string()),
                };

                match self.send_core_control(core_message).await {
                    Ok(response) => match native_result_for_core_response(&command, response) {
                        Ok(result) => success_response(id, result),
                        Err(err) => error_response(Some(id), err.code(), err.to_string()),
                    },
                    Err(err) => error_response(Some(id), err.code(), err.to_string()),
                }
            }
            NativeCommand::Dispatch { .. } => {
                let dispatch_request = match frontend_dispatch_request_for_command(&request.command)
                {
                    Ok(dispatch_request) => dispatch_request,
                    Err(err) => return error_response(Some(id), err.code(), err.to_string()),
                };

                match self.send_frontend_dispatch(dispatch_request).await {
                    Ok(FrontendDispatchResponse::PlatformError { code, message }) => {
                        error_response(Some(id), code, message)
                    }
                    Ok(response) => match native_result_for_frontend_dispatch_response(response) {
                        Ok(result) => success_response(id, result),
                        Err(err) => error_response(Some(id), err.code(), err.to_string()),
                    },
                    Err(err) => error_response(Some(id), err.code(), err.to_string()),
                }
            }
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

    async fn send_frontend_dispatch(
        &mut self,
        request: FrontendDispatchRequest,
    ) -> Result<FrontendDispatchResponse> {
        self.ensure_connection().await?;
        let result = self
            .send_frontend_dispatch_on_cached_connection(request)
            .await;

        if result.is_err() {
            self.connection = None;
        }

        result
    }

    async fn send_frontend_dispatch_on_cached_connection(
        &mut self,
        request: FrontendDispatchRequest,
    ) -> Result<FrontendDispatchResponse> {
        let request_id = self.next_request_id();
        let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(request))
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

        match decode_frontend_dispatch_message(&payload)
            .map_err(|err| NativeHostError::CoreIpc(err.to_string()))?
        {
            FrontendDispatchMessage::DispatchResult(response) => Ok(response),
            message => Err(NativeHostError::UnexpectedCoreResponse(format!(
                "expected frontend dispatch result, got {message:?}"
            ))),
        }
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}
