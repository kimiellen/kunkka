use crate::{AppId, Result, WorkerSdkError};
use kunkka_ipc::{
    EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId, StreamId,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CAPABILITY_CONTENT_TYPE: &str = "application/vnd.kunkka.capability.v1+postcard";
pub const CAPABILITY_SCHEMA: &str = "kunkka.capability.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub app_id: String,
    pub capability: String,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub result: std::result::Result<Vec<u8>, CapabilityError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityStreamChunk {
    pub bytes: Vec<u8>,
    pub metadata: FrameMetadata,
    pub end: bool,
}

pub struct CapabilityStream {
    connection: IpcConnection,
    request_id: RequestId,
    session_id: SessionId,
    stream_id: Option<StreamId>,
    buffered: Option<CapabilityStreamChunk>,
}

pub fn encode_capability_request(request: &CapabilityRequest) -> Result<Payload> {
    let bytes = postcard::to_stdvec(request)?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_request(payload: &Payload) -> Result<CapabilityRequest> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}

pub fn encode_capability_response(response: &CapabilityResponse) -> Result<Payload> {
    let bytes = postcard::to_stdvec(response)?;
    Ok(Payload {
        bytes,
        content_type: Some(CAPABILITY_CONTENT_TYPE.to_string()),
        schema: Some(CAPABILITY_SCHEMA.to_string()),
        metadata: FrameMetadata::new(),
    })
}

pub fn decode_capability_response(payload: &Payload) -> Result<CapabilityResponse> {
    Ok(postcard::from_bytes(&payload.bytes)?)
}

pub async fn call_capability(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    capability: &str,
    method: &str,
    params: Vec<u8>,
) -> Result<CapabilityResponse> {
    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: app_id.as_str().to_string(),
        capability: capability.to_string(),
        method: method.to_string(),
        params,
    })?;

    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("worker-sdk"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    let mut connection = IpcConnection::connect(socket_path).await?;
    connection.send_frame(&frame).await?;

    let response = connection
        .recv_frame()
        .await?
        .ok_or(kunkka_ipc::IpcError::ConnectionClosed)?;

    let Frame::Response {
        request_id: response_request_id,
        payload,
        ..
    } = response
    else {
        return Err(WorkerSdkError::Protocol(
            "expected capability response frame".to_string(),
        ));
    };

    if response_request_id != request_id {
        return Err(WorkerSdkError::Protocol(format!(
            "response request_id mismatch: expected {}, got {}",
            request_id.0, response_request_id.0
        )));
    }

    decode_capability_response(&payload)
}

pub async fn open_capability_stream(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    capability: &str,
    method: &str,
    params: Vec<u8>,
) -> Result<CapabilityStream> {
    let request_id = RequestId(1);
    let session_id = SessionId(1);
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: app_id.as_str().to_string(),
        capability: capability.to_string(),
        method: method.to_string(),
        params,
    })?;

    let frame = Frame::Request {
        request_id,
        session_id,
        source: EndpointId::new("worker-sdk"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    };

    let mut connection = IpcConnection::connect(socket_path).await?;
    connection.send_frame(&frame).await?;

    let first = connection
        .recv_frame()
        .await?
        .ok_or(kunkka_ipc::IpcError::ConnectionClosed)?;

    match first {
        Frame::Stream {
            stream_id,
            request_id: response_request_id,
            session_id: response_session_id,
            payload,
            end,
            ..
        } => {
            if response_request_id != Some(request_id) || response_session_id != session_id {
                return Err(WorkerSdkError::Protocol(
                    "stream request/session mismatch".to_string(),
                ));
            }

            Ok(CapabilityStream {
                connection,
                request_id,
                session_id,
                stream_id: Some(stream_id),
                buffered: Some(CapabilityStreamChunk {
                    bytes: payload.bytes,
                    metadata: payload.metadata,
                    end,
                }),
            })
        }
        Frame::Response {
            request_id: response_request_id,
            payload,
            ..
        } => {
            if response_request_id != request_id {
                return Err(WorkerSdkError::Protocol(format!(
                    "response request_id mismatch: expected {}, got {}",
                    request_id.0, response_request_id.0
                )));
            }

            let response = decode_capability_response(&payload)?;
            match response.result {
                Ok(_) => Err(WorkerSdkError::Protocol(
                    "expected stream frame, got response frame".to_string(),
                )),
                Err(err) => Err(WorkerSdkError::Protocol(format!(
                    "capability stream rejected: {}: {}",
                    err.code, err.message
                ))),
            }
        }
        Frame::Error { code, message, .. } => Err(WorkerSdkError::Protocol(format!(
            "capability stream failed: {code}: {message}"
        ))),
        _ => Err(WorkerSdkError::Protocol(
            "expected capability stream frame".to_string(),
        )),
    }
}

impl CapabilityStream {
    pub async fn next_chunk(&mut self) -> Result<Option<CapabilityStreamChunk>> {
        if let Some(chunk) = self.buffered.take() {
            return Ok(Some(chunk));
        }

        let frame = match self.connection.recv_frame().await? {
            Some(frame) => frame,
            None => return Ok(None),
        };

        match frame {
            Frame::Stream {
                stream_id,
                request_id,
                session_id,
                payload,
                end,
                ..
            } => {
                if Some(stream_id) != self.stream_id
                    || request_id != Some(self.request_id)
                    || session_id != self.session_id
                {
                    return Err(WorkerSdkError::Protocol(
                        "stream request/session mismatch".to_string(),
                    ));
                }

                Ok(Some(CapabilityStreamChunk {
                    bytes: payload.bytes,
                    metadata: payload.metadata,
                    end,
                }))
            }
            Frame::Error {
                request_id,
                stream_id,
                code,
                message,
                ..
            } => {
                if request_id != Some(self.request_id) && stream_id != self.stream_id {
                    return Err(WorkerSdkError::Protocol(
                        "unexpected error frame for another stream".to_string(),
                    ));
                }
                Err(WorkerSdkError::Protocol(format!(
                    "capability stream failed: {code}: {message}"
                )))
            }
            _ => Err(WorkerSdkError::Protocol(
                "expected capability stream frame".to_string(),
            )),
        }
    }
}
