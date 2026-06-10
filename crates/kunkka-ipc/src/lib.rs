pub mod codec;
pub mod error;
pub mod frame;
pub mod transport;

pub use codec::{decode_frame, encode_frame, DEFAULT_MAX_FRAME_LEN};
pub use error::IpcError;
pub use frame::{EndpointId, Frame, FrameMetadata, Payload, RequestId, SessionId, StreamId};
pub use transport::{IpcConnection, IpcListener};
