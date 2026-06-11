use crate::{NativeHostError, Result};
use serde::Serialize;
use std::io::{ErrorKind, Read, Write};

pub const MAX_NATIVE_MESSAGE_LEN: usize = 1024 * 1024;

pub fn read_native_message<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_bytes = [0_u8; 4];
    let mut read_len = 0;

    while read_len < len_bytes.len() {
        let bytes_read = reader.read(&mut len_bytes[read_len..])?;
        if bytes_read == 0 {
            if read_len == 0 {
                return Ok(None);
            }

            return Err(NativeHostError::InvalidRequest(
                "native message length prefix ended early".to_string(),
            ));
        }

        read_len += bytes_read;
    }

    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_NATIVE_MESSAGE_LEN {
        return Err(NativeHostError::InvalidRequest(format!(
            "native message too large: {len} bytes"
        )));
    }

    let mut body = vec![0_u8; len];
    reader.read_exact(&mut body).map_err(|err| {
        if err.kind() == ErrorKind::UnexpectedEof {
            NativeHostError::InvalidRequest(
                "native message body ended before declared length".to_string(),
            )
        } else {
            NativeHostError::Io(err)
        }
    })?;
    Ok(Some(body))
}

pub fn write_native_message<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_NATIVE_MESSAGE_LEN {
        return Err(NativeHostError::InvalidRequest(format!(
            "native message too large: {} bytes",
            body.len()
        )));
    }

    writer.write_all(&(body.len() as u32).to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}
