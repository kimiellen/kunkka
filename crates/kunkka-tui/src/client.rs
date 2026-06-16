use std::path::PathBuf;

use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CorePingRequest,
    CorePingResponse,
};

use crate::error::TuiError;

pub fn resolve_socket_path() -> PathBuf {
    if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(&xdg_runtime);
        if path.is_absolute() {
            return path.join("kunkka").join("core.sock");
        }
    }
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
}

pub async fn ping_core(socket_path: &PathBuf) -> Result<CorePingResponse, TuiError> {
    let mut connection = IpcConnection::connect(socket_path)
        .await
        .map_err(|e| TuiError::CoreUnavailable(e.to_string()))?;

    let payload = encode_control_message(&CoreControlMessage::Ping(CorePingRequest {}))
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let request = Frame::Request {
        request_id: RequestId(1),
        session_id: SessionId(1),
        source: EndpointId::new("tui"),
        target: EndpointId::new("core"),
        metadata: FrameMetadata::new(),
        payload,
    };

    connection
        .send_frame(&request)
        .await
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let response = connection
        .recv_frame()
        .await
        .map_err(|e| TuiError::CoreIpc(e.to_string()))?;

    let frame = response
        .ok_or_else(|| TuiError::CoreIpc("connection closed before response".to_string()))?;

    match frame {
        Frame::Response {
            request_id,
            payload,
            ..
        } => {
            if request_id != RequestId(1) {
                return Err(TuiError::UnexpectedCoreResponse(format!(
                    "expected request_id 1, got {request_id:?}"
                )));
            }
            match decode_control_message(&payload) {
                Ok(CoreControlMessage::Pong(pong)) => Ok(pong),
                Ok(other) => Err(TuiError::UnexpectedCoreResponse(format!(
                    "expected Pong, got {other:?}"
                ))),
                Err(e) => Err(TuiError::CoreIpc(e.to_string())),
            }
        }
        other => Err(TuiError::UnexpectedCoreResponse(format!(
            "expected Response frame, got {other:?}"
        ))),
    }
}
