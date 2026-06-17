use kunkka_cli::cli::{ApprovalCommand, CliCommand};
use kunkka_cli::client::core_message_for_command;
use kunkka_protocol::core_control::CoreControlMessage;

#[test]
fn ping_command_maps_to_core_ping() {
    let message = core_message_for_command(&CliCommand::Ping).unwrap();
    assert!(matches!(message, CoreControlMessage::Ping(_)));
}

#[test]
fn status_command_maps_to_core_status() {
    let message = core_message_for_command(&CliCommand::Status).unwrap();
    assert!(matches!(message, CoreControlMessage::Status(_)));
}

#[test]
fn approvals_list_command_maps_to_core_list_pending_approvals() {
    let message = core_message_for_command(&CliCommand::Approvals {
        command: ApprovalCommand::List,
    })
    .unwrap();
    assert!(matches!(
        message,
        CoreControlMessage::ListPendingApprovals(_)
    ));
}

#[test]
fn approvals_approve_command_maps_to_core_approve_pending_approval() {
    let message = core_message_for_command(&CliCommand::Approvals {
        command: ApprovalCommand::Approve {
            approval_id: "appr_1".to_string(),
        },
    })
    .unwrap();
    assert!(matches!(
        message,
        CoreControlMessage::ApprovePendingApproval(_)
    ));
}

#[test]
fn approvals_reject_command_maps_to_core_reject_pending_approval() {
    let message = core_message_for_command(&CliCommand::Approvals {
        command: ApprovalCommand::Reject {
            approval_id: "appr_1".to_string(),
        },
    })
    .unwrap();
    assert!(matches!(
        message,
        CoreControlMessage::RejectPendingApproval(_)
    ));
}

#[test]
fn dispatch_command_returns_none_for_control() {
    let result = core_message_for_command(&CliCommand::Dispatch {
        app_id: "notes".to_string(),
        method: "search".to_string(),
        payload: serde_json::json!({}),
    });
    assert!(result.is_none());
}
