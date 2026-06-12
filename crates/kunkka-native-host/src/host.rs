use crate::bridge::NativeHostSession;
use crate::native_messaging::{read_native_message, write_native_message};
use crate::native_protocol::{decode_request, error_response, extract_request_id, NativeErrorCode};
use crate::{NativeHostError, Result};
use std::io::{Read, Write};

pub async fn run_native_host<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    session: &mut NativeHostSession,
) -> Result<()> {
    loop {
        let message_bytes = match read_native_message(reader) {
            Ok(Some(message_bytes)) => message_bytes,
            Ok(None) => return Ok(()),
            Err(NativeHostError::InvalidRequest(message)) => {
                let response = error_response(None, NativeErrorCode::InvalidRequest, message);
                write_native_message(writer, &response)?;
                continue;
            }
            Err(err) => return Err(err),
        };

        let response = match decode_request(&message_bytes) {
            Ok(request) => session.handle_request(request).await,
            Err(err) => {
                let message = normalize_invalid_request_message(&err);
                error_response(
                    extract_request_id(&message_bytes),
                    NativeErrorCode::InvalidRequest,
                    message,
                )
            }
        };

        write_native_message(writer, &response)?;
    }
}

fn normalize_invalid_request_message(err: &NativeHostError) -> String {
    match err {
        NativeHostError::Json(json_err) if json_err.to_string().contains("missing field `id`") => {
            "missing request id".to_string()
        }
        NativeHostError::InvalidRequest(message) => message.clone(),
        NativeHostError::Json(json_err) => json_err.to_string(),
        other => other.to_string(),
    }
}
