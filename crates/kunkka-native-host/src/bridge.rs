use crate::native_protocol::{NativeCommand, NativeResult};
use crate::{NativeHostError, Result};
use kunkka_protocol::core_control::{
    CoreControlMessage, CorePingRequest, CorePingResponse, CoreStatusRequest,
};

pub fn core_message_for_command(command: &NativeCommand) -> CoreControlMessage {
    match command {
        NativeCommand::Ping => CoreControlMessage::Ping(CorePingRequest),
        NativeCommand::Status => CoreControlMessage::Status(CoreStatusRequest),
    }
}

pub fn native_result_for_core_response(
    command: &NativeCommand,
    message: CoreControlMessage,
) -> Result<NativeResult> {
    match (command, message) {
        (NativeCommand::Ping, CoreControlMessage::Pong(CorePingResponse)) => Ok(NativeResult::Pong),
        (NativeCommand::Status, CoreControlMessage::StatusResult(status)) => {
            Ok(NativeResult::Status {
                worker_count: status.worker_count,
                socket_path: status.socket_path,
                runtime_ready: status.runtime_ready,
            })
        }
        (command, message) => Err(NativeHostError::UnexpectedCoreResponse(format!(
            "unexpected core response for {command:?}: {message:?}"
        ))),
    }
}
