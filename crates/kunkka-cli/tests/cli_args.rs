use clap::Parser;
use kunkka_cli::cli::{ApprovalCommand, Cli, CliCommand};

#[test]
fn parses_ping_command() {
    let cli = Cli::try_parse_from(["kunkka", "ping"]).unwrap();
    assert!(matches!(cli.command, CliCommand::Ping));
}

#[test]
fn parses_status_command() {
    let cli = Cli::try_parse_from(["kunkka", "status"]).unwrap();
    assert!(matches!(cli.command, CliCommand::Status));
}

#[test]
fn parses_dispatch_command() {
    let cli = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "search",
        "--payload",
        r#"{"query":"kunkka"}"#,
    ])
    .unwrap();
    match cli.command {
        CliCommand::Dispatch {
            app_id,
            method,
            payload,
        } => {
            assert_eq!(app_id, "notes");
            assert_eq!(method, "search");
            assert_eq!(payload, serde_json::json!({"query": "kunkka"}));
        }
        _ => panic!("expected dispatch command"),
    }
}

#[test]
fn parses_shell_command() {
    let cli = Cli::try_parse_from([
        "kunkka",
        "shell",
        "--app",
        "notes",
        "--command",
        "printf foo | wc -c",
    ])
    .unwrap();

    match cli.command {
        CliCommand::Shell { app_id, command } => {
            assert_eq!(app_id, "notes");
            assert_eq!(command, "printf foo | wc -c");
        }
        other => panic!("expected shell command, got {other:?}"),
    }
}

#[test]
fn parses_approvals_list_command() {
    let cli = Cli::try_parse_from(["kunkka", "approvals", "list"]).unwrap();
    assert!(matches!(
        cli.command,
        CliCommand::Approvals {
            command: ApprovalCommand::List
        }
    ));
}

#[test]
fn parses_approvals_approve_command() {
    let cli = Cli::try_parse_from(["kunkka", "approvals", "approve", "--id", "appr_1"]).unwrap();
    match cli.command {
        CliCommand::Approvals {
            command: ApprovalCommand::Approve { approval_id },
        } => {
            assert_eq!(approval_id, "appr_1");
        }
        _ => panic!("expected approvals approve command"),
    }
}

#[test]
fn parses_approvals_reject_command() {
    let cli = Cli::try_parse_from(["kunkka", "approvals", "reject", "--id", "appr_1"]).unwrap();
    match cli.command {
        CliCommand::Approvals {
            command: ApprovalCommand::Reject { approval_id },
        } => {
            assert_eq!(approval_id, "appr_1");
        }
        _ => panic!("expected approvals reject command"),
    }
}

#[test]
fn rejects_dispatch_missing_app() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--method",
        "search",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_missing_method() {
    let result = Cli::try_parse_from(["kunkka", "dispatch", "--app", "notes", "--payload", "{}"]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_invalid_json_payload() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "search",
        "--payload",
        "not json",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_empty_app() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "",
        "--method",
        "search",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_dispatch_empty_method() {
    let result = Cli::try_parse_from([
        "kunkka",
        "dispatch",
        "--app",
        "notes",
        "--method",
        "",
        "--payload",
        "{}",
    ]);
    assert!(result.is_err());
}

#[test]
fn rejects_approvals_empty_id() {
    let result = Cli::try_parse_from(["kunkka", "approvals", "approve", "--id", ""]);
    assert!(result.is_err());
}
