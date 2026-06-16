use clap::Parser;
use kunkka_cli::cli::{Cli, CliCommand};

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
