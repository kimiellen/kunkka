use crate::cli::CliCommand;
use crate::error::CliError;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CoreStatusRequest,
};
use kunkka_protocol::frontend_dispatch::{
    decode_frontend_dispatch_message, encode_frontend_dispatch_message, FrontendDispatchMessage,
    FrontendDispatchRequest, FrontendDispatchResponse,
};
use std::path::Path;

const JSON_CONTENT_TYPE: &str = "application/json";

pub fn core_message_for_command(command: &CliCommand) -> Option<CoreControlMessage> {
    match command {
        CliCommand::Ping => Some(CoreControlMessage::Ping(CorePingRequest)),
        CliCommand::Status => Some(CoreControlMessage::Status(CoreStatusRequest)),
        CliCommand::Dispatch { .. } => None,
    }
}

pub async fn send_core_control(
    socket_path: &Path,
    message: CoreControlMessage,
) -> Result<CoreControlMessage, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload =
        encode_control_message(&message).map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_control_message(&payload).map_err(|err| CliError::CoreIpc(err.to_string()))
}

pub async fn send_frontend_dispatch(
    socket_path: &Path,
    request: FrontendDispatchRequest,
) -> Result<FrontendDispatchResponse, CliError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|err| CliError::CoreUnavailable(err.to_string()))?;

    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_frontend_dispatch_message(&FrontendDispatchMessage::Dispatch(request))
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;
    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("cli"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    connection
        .send_frame(&frame)
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
        .ok_or_else(|| CliError::CoreIpc("core closed connection".to_string()))?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(CliError::UnexpectedCoreResponse(
            "expected response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(CliError::UnexpectedCoreResponse(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    match decode_frontend_dispatch_message(&payload)
        .map_err(|err| CliError::CoreIpc(err.to_string()))?
    {
        FrontendDispatchMessage::DispatchResult(response) => Ok(response),
        _ => Err(CliError::UnexpectedCoreResponse(
            "expected frontend dispatch result".to_string(),
        )),
    }
}

pub fn build_frontend_dispatch_request(
    app_id: String,
    method: String,
    payload: serde_json::Value,
) -> FrontendDispatchRequest {
    FrontendDispatchRequest {
        app_id,
        method,
        payload: Payload {
            bytes: serde_json::to_vec(&payload).unwrap_or_default(),
            content_type: Some(JSON_CONTENT_TYPE.to_string()),
            schema: None,
            metadata: FrameMetadata::new(),
        },
    }
}
