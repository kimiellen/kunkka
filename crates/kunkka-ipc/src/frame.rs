use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type FrameMetadata = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StreamId(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u128);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EndpointId(String);

impl EndpointId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub schema: Option<String>,
    pub metadata: FrameMetadata,
}

impl Payload {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            content_type: None,
            schema: None,
            metadata: FrameMetadata::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Request {
        request_id: RequestId,
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        payload: Payload,
        metadata: FrameMetadata,
    },
    Response {
        request_id: RequestId,
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        payload: Payload,
        metadata: FrameMetadata,
    },
    Event {
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        name: String,
        payload: Payload,
        metadata: FrameMetadata,
    },
    Stream {
        stream_id: StreamId,
        request_id: Option<RequestId>,
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        payload: Payload,
        end: bool,
        metadata: FrameMetadata,
    },
    Cancel {
        request_id: Option<RequestId>,
        stream_id: Option<StreamId>,
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        reason: Option<String>,
        metadata: FrameMetadata,
    },
    Heartbeat {
        session_id: SessionId,
        source: EndpointId,
        target: EndpointId,
        metadata: FrameMetadata,
    },
    Error {
        request_id: Option<RequestId>,
        stream_id: Option<StreamId>,
        session_id: Option<SessionId>,
        source: EndpointId,
        target: EndpointId,
        code: String,
        message: String,
        metadata: FrameMetadata,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frame_roundtrips_with_opaque_payload() {
        let frame = Frame::Request {
            request_id: RequestId(1),
            session_id: SessionId(2),
            source: EndpointId::new("native-host"),
            target: EndpointId::new("core"),
            payload: Payload::from_bytes(b"hello".to_vec()),
            metadata: FrameMetadata::new(),
        };

        let encoded = postcard::to_stdvec(&frame).unwrap();
        let decoded: Frame = postcard::from_bytes(&encoded).unwrap();

        assert_eq!(decoded, frame);
    }
}
