use crate::codec::{decode_frame, encode_frame_with_limit, DEFAULT_MAX_FRAME_LEN};
use crate::{Frame, IpcError};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use tokio::net::{UnixListener, UnixStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub struct IpcConnection {
    inner: Framed<UnixStream, LengthDelimitedCodec>,
    max_frame_len: usize,
}

impl IpcConnection {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, IpcError> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self::from_stream(stream))
    }

    pub fn from_stream(stream: UnixStream) -> Self {
        Self::from_stream_with_limit(stream, DEFAULT_MAX_FRAME_LEN)
    }

    pub fn from_stream_with_limit(stream: UnixStream, max_frame_len: usize) -> Self {
        let mut codec = LengthDelimitedCodec::new();
        codec.set_max_frame_length(max_frame_len);

        Self {
            inner: Framed::new(stream, codec),
            max_frame_len,
        }
    }

    pub async fn send_frame(&mut self, frame: &Frame) -> Result<(), IpcError> {
        let bytes = encode_frame_with_limit(frame, self.max_frame_len)?;
        self.inner.send(Bytes::from(bytes)).await?;
        Ok(())
    }

    pub async fn recv_frame(&mut self) -> Result<Option<Frame>, IpcError> {
        let Some(bytes) = self.inner.next().await else {
            return Ok(None);
        };

        let bytes = bytes?;
        Ok(Some(decode_frame(&bytes)?))
    }
}

pub struct IpcListener {
    inner: UnixListener,
    max_frame_len: usize,
}

impl IpcListener {
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self, IpcError> {
        let inner = UnixListener::bind(path)?;
        Ok(Self {
            inner,
            max_frame_len: DEFAULT_MAX_FRAME_LEN,
        })
    }

    pub async fn accept(&self) -> Result<IpcConnection, IpcError> {
        let (stream, _) = self.inner.accept().await?;
        Ok(IpcConnection::from_stream_with_limit(
            stream,
            self.max_frame_len,
        ))
    }
}
