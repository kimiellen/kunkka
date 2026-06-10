use crate::{Frame, IpcError};

pub const DEFAULT_MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, IpcError> {
    encode_frame_with_limit(frame, DEFAULT_MAX_FRAME_LEN)
}

pub fn encode_frame_with_limit(frame: &Frame, max_frame_len: usize) -> Result<Vec<u8>, IpcError> {
    let bytes = postcard::to_stdvec(frame)?;

    if bytes.len() > max_frame_len {
        return Err(IpcError::FrameTooLarge {
            size: bytes.len(),
            max: max_frame_len,
        });
    }

    Ok(bytes)
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, IpcError> {
    Ok(postcard::from_bytes(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_malformed_bytes_returns_error() {
        let err = decode_frame(&[0xff, 0x00, 0xff]).unwrap_err();
        assert!(matches!(err, crate::IpcError::Codec(_)));
    }
}
