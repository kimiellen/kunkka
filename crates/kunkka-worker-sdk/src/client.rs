use crate::{
    decode_worker_message, encode_worker_message, RegisterWorkerRequest, RegisterWorkerResponse,
    Result, WorkerId, WorkerProtocolMessage, WorkerSdkError,
};
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use std::path::Path;

pub struct WorkerClient {
    connection: IpcConnection,
    worker_endpoint: EndpointId,
    core_endpoint: EndpointId,
    session_id: SessionId,
    next_request_id: u128,
}

impl WorkerClient {
    pub async fn connect(path: impl AsRef<Path>, worker_id: WorkerId) -> Result<Self> {
        let connection = IpcConnection::connect(path).await?;
        Ok(Self::from_connection(connection, worker_id, SessionId(1)))
    }

    pub fn from_connection(
        connection: IpcConnection,
        worker_id: WorkerId,
        session_id: SessionId,
    ) -> Self {
        Self {
            connection,
            worker_endpoint: EndpointId::new(format!("worker:{}", worker_id.as_str())),
            core_endpoint: EndpointId::new("core"),
            session_id,
            next_request_id: 1,
        }
    }

    pub async fn register(
        &mut self,
        request: RegisterWorkerRequest,
    ) -> Result<RegisterWorkerResponse> {
        let request_id = self.next_request_id();
        let payload = encode_worker_message(&WorkerProtocolMessage::RegisterWorker(request))?;

        let frame = Frame::Request {
            request_id,
            session_id: self.session_id,
            source: self.worker_endpoint.clone(),
            target: self.core_endpoint.clone(),
            payload,
            metadata: FrameMetadata::new(),
        };

        self.connection.send_frame(&frame).await?;

        let response = self
            .connection
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
                "expected registration response frame".to_string(),
            ));
        };

        if response_request_id != request_id {
            return Err(WorkerSdkError::Protocol(format!(
                "response request_id mismatch: expected {}, got {}",
                request_id.0, response_request_id.0
            )));
        }

        let message = decode_worker_message(&payload)?;

        match message {
            WorkerProtocolMessage::RegisterWorkerAccepted(response) => Ok(response),
            other => Err(WorkerSdkError::Protocol(format!(
                "expected RegisterWorkerAccepted, got {other:?}"
            ))),
        }
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id += 1;
        request_id
    }
}
