use kunkka_cli::cli::{ApprovalCommand, Cli, CliCommand};
use kunkka_cli::run_command_with_socket;
use kunkka_cli::run_command_with_socket_and_input;
use kunkka_core::capability::shell::{
    PendingApprovalReceipt, ShellRunOutcome, ShellRunParams, ShellRunResult,
};
use kunkka_core::prepare_core_runtime;
use kunkka_core::runtime::CoreRuntime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, Payload, RequestId, SessionId};
use kunkka_worker_sdk::capability::{
    decode_capability_response, encode_capability_request, CapabilityRequest,
};
use kunkka_worker_sdk::{
    AppId, DispatchWorkerResponse, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId,
};
use std::future::Future;
use std::time::Duration;
use tempfile::tempdir;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(TEST_TIMEOUT, future)
        .await
        .expect("test operation timed out")
}

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempdir().unwrap();
    let paths = KunkkaPaths {
        config_dir: root.path().join("config"),
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: root.path().join("runtime/core.sock"),
    };
    (root, paths)
}

fn write_manifest(config_dir: &std::path::Path, body: &str) {
    let apps_dir = config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    std::fs::write(apps_dir.join("notes.json"), body).unwrap();
}

fn write_manifest_with_shell(paths: &KunkkaPaths, allow: &[&str], ask: &[&str]) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();

    let allow_json = allow
        .iter()
        .map(|command| format!("\"{command}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let ask_json = ask
        .iter()
        .map(|command| format!("\"{command}\""))
        .collect::<Vec<_>>()
        .join(", ");

    std::fs::write(
        apps_dir.join("notes.json"),
        format!(
            r#"{{
                "app_id": "notes",
                "worker": {{
                    "program": "/usr/bin/notes-worker",
                    "args": ["--serve"]
                }},
                "capabilities": {{
                    "shell": {{
                        "allow": [{allow_json}],
                        "ask": [{ask_json}]
                    }}
                }}
            }}"#,
        ),
    )
    .unwrap();
}

fn capability_frame(request_id: u128, params: &ShellRunParams) -> Frame {
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: "notes".to_string(),
        capability: "shell".to_string(),
        method: "run".to_string(),
        params: postcard::to_stdvec(params).unwrap(),
    })
    .unwrap();

    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("worker:notes"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

fn decode_shell_outcome(frame: Frame) -> ShellRunOutcome {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    let bytes = decode_capability_response(&payload)
        .unwrap()
        .result
        .unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

async fn run_runtime_until_cli_task_completes(
    mut runtime: CoreRuntime,
    mut cli_task: tokio::task::JoinHandle<
        Result<kunkka_cli::output::CliOutput, kunkka_cli::error::CliError>,
    >,
) -> Result<kunkka_cli::output::CliOutput, kunkka_cli::error::CliError> {
    loop {
        tokio::select! {
            result = runtime.run_once() => {
                result.unwrap();
            }
            output = &mut cli_task => {
                return output.unwrap();
            }
        }
    }
}

#[tokio::test]
async fn cli_ping_returns_pong() {
    let (_root, paths) = test_paths();
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Ping,
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task)
        .await
        .unwrap();
    assert!(result.is_success());
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({"ok":true,"result":{"type":"pong"}})
    );
}

#[tokio::test]
async fn cli_status_returns_status() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Status,
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "status");
    assert_eq!(value["result"]["worker_count"], 0);
    assert!(value["result"]["socket_path"].as_str().is_some());
    assert_eq!(value["result"]["runtime_ready"], true);
}

#[tokio::test]
async fn cli_core_unavailable_returns_error() {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("nonexistent.sock");

    let cli = Cli {
        command: CliCommand::Ping,
    };
    let result = run_command_with_socket(&cli, &socket_path).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "core_unavailable");
}

#[tokio::test]
async fn cli_dispatch_returns_worker_payload() {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("core.sock");
    let config_dir = root.path().join("config");

    write_manifest(
        &config_dir,
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "permissions": {
                "frontend_dispatch": {
                    "allowed_methods": ["search"]
                }
            }
        }"#,
    );

    let paths = KunkkaPaths {
        config_dir,
        data_dir: root.path().join("data"),
        state_dir: root.path().join("state"),
        cache_dir: root.path().join("cache"),
        runtime_dir: root.path().join("runtime"),
        database_path: root.path().join("data/kunkka.db"),
        log_dir: root.path().join("state/logs"),
        socket_path: socket_path.clone(),
    };
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            let registration = client
                .register(RegisterWorkerRequest {
                    worker_id: WorkerId::new("notes"),
                    app_id: AppId::new("notes"),
                    capabilities: vec![WorkerCapability {
                        name: "notes.search".to_string(),
                        description: None,
                    }],
                })
                .await
                .unwrap();
            let request =
                tokio::time::timeout(std::time::Duration::from_secs(5), client.recv_dispatch())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(request.request.app_id.as_str(), "notes");
            assert_eq!(request.request.method, "search");
            client
                .respond_dispatch(
                    request,
                    DispatchWorkerResponse::Ok(Payload {
                        bytes: br#"{"items":["a","b"]}"#.to_vec(),
                        content_type: Some("application/json".to_string()),
                        schema: None,
                        metadata: FrameMetadata::new(),
                    }),
                )
                .await
                .unwrap();
            registration
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let cli_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Dispatch {
                    app_id: "notes".to_string(),
                    method: "search".to_string(),
                    payload: serde_json::json!({"query": "kunkka"}),
                },
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = cli_task.await.unwrap().unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "dispatch");
    assert_eq!(
        value["result"]["payload"]["items"],
        serde_json::json!(["a", "b"])
    );

    let registration = tokio::time::timeout(std::time::Duration::from_secs(5), worker_task)
        .await
        .unwrap()
        .unwrap();
    assert!(registration.accepted);
}

#[tokio::test]
async fn cli_approvals_list_returns_pending_item() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf approved".to_string(),
            approval_id: None,
        },
    );
    let pending_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&pending_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let pending = decode_shell_outcome(wait_for(pending_task).await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        _ => panic!("expected pending approval receipt"),
    };

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Approvals {
                    command: ApprovalCommand::List,
                },
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = wait_for(cli_task).await.unwrap().unwrap();
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "pending_approvals");
    assert_eq!(value["result"]["approvals"][0]["approval_id"], approval_id);
    assert_eq!(value["result"]["approvals"][0]["app_id"], "notes");
    assert_eq!(value["result"]["approvals"][0]["capability"], "shell");
}

#[tokio::test]
async fn cli_approvals_approve_allows_retry() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf approved".to_string(),
            approval_id: None,
        },
    );
    let pending_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&pending_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let pending = decode_shell_outcome(wait_for(pending_task).await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        _ => panic!("expected pending approval receipt"),
    };

    let approve_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        let approval_id = approval_id.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Approvals {
                    command: ApprovalCommand::Approve { approval_id },
                },
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let approve_result = wait_for(approve_task).await.unwrap().unwrap();
    let approve_value = serde_json::to_value(&approve_result).unwrap();
    assert_eq!(approve_value["ok"], true);
    assert_eq!(approve_value["result"]["type"], "approval_decision");

    let retry_frame = capability_frame(
        2,
        &ShellRunParams {
            command: "printf approved".to_string(),
            approval_id: Some(approval_id),
        },
    );
    let retry_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&retry_frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let retried = decode_shell_outcome(wait_for(retry_task).await.unwrap());
    let ShellRunOutcome::Completed(ShellRunResult {
        stdout, exit_code, ..
    }) = retried
    else {
        panic!("expected completed shell result after approval");
    };
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, "approved");
}

#[tokio::test]
async fn cli_shell_completed_outputs_result() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &["printf", "wc"], &[]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf foo | wc -c".to_string(),
                },
            };
            run_command_with_socket(&cli, &socket_path).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task)
        .await
        .unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "shell_result");
    assert_eq!(value["result"]["exit_code"], 0);
    assert_eq!(value["result"]["stdout"].as_str().unwrap().trim(), "3");
}

#[tokio::test]
async fn cli_shell_pending_approve_retries_and_completes() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf approved".to_string(),
                },
            };
            run_command_with_socket_and_input(&cli, &socket_path, &b"y\n"[..]).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task)
        .await
        .unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "shell_result");
    assert_eq!(value["result"]["exit_code"], 0);
    assert_eq!(value["result"]["stdout"], "approved");
}

#[tokio::test]
async fn cli_shell_pending_approve_yes_retries_and_completes() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf approved".to_string(),
                },
            };
            run_command_with_socket_and_input(&cli, &socket_path, &b"yes\n"[..]).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task)
        .await
        .unwrap();
    assert!(result.is_success());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["type"], "shell_result");
    assert_eq!(value["result"]["exit_code"], 0);
    assert_eq!(value["result"]["stdout"], "approved");
}

#[tokio::test]
async fn cli_shell_pending_reject_returns_error() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf approved".to_string(),
                },
            };
            run_command_with_socket_and_input(&cli, &socket_path, &b"n\n"[..]).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "approval_rejected");
    assert!(
        err.to_string().contains("approval rejected by user"),
        "error message should mention rejection: {}",
        err
    );
}

#[tokio::test]
async fn cli_shell_pending_empty_input_rejects() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf approved".to_string(),
                },
            };
            // Empty line (just pressing Enter) should reject (default N)
            run_command_with_socket_and_input(&cli, &socket_path, &b"\n"[..]).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "approval_rejected");
}

#[tokio::test]
async fn cli_shell_pending_eof_rejects() {
    let (_root, paths) = test_paths();
    write_manifest_with_shell(&paths, &[], &["printf"]);
    let runtime = prepare_core_runtime(&paths).await.unwrap();

    let cli_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let cli = Cli {
                command: CliCommand::Shell {
                    app_id: "notes".to_string(),
                    command: "printf approved".to_string(),
                },
            };
            // Empty input (immediate EOF) should reject
            run_command_with_socket_and_input(&cli, &socket_path, &b""[..]).await
        }
    });

    let result = run_runtime_until_cli_task_completes(runtime, cli_task).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "approval_rejected");
}
