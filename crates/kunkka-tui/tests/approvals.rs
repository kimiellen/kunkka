use std::time::Duration;

use kunkka_core::capability::shell::{PendingApprovalReceipt, ShellRunOutcome, ShellRunParams};
use kunkka_core::capability::{
    decode_capability_response, encode_capability_request, CapabilityRequest,
};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use kunkka_tui::app::{App, ApprovalsStatus, PendingApprovalItem, View};
use kunkka_tui::client;

fn test_paths() -> (tempfile::TempDir, KunkkaPaths) {
    let root = tempfile::tempdir().unwrap();
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

fn write_manifest_with_ask(paths: &KunkkaPaths) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
    std::fs::write(
        apps_dir.join("notes.json"),
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            },
            "capabilities": {
                "shell": {
                    "allow": [],
                    "ask": ["printf"]
                }
            }
        }"#,
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

/// Helper: start runtime with an "ask" manifest, send a capability request
/// to create a pending approval, and return the approval_id.
async fn setup_pending_approval(
    paths: &KunkkaPaths,
) -> (kunkka_core::runtime::CoreRuntime, String) {
    write_manifest_with_ask(paths);
    let mut runtime = prepare_core_runtime(paths).await.unwrap();

    let pending_frame = capability_frame(
        1,
        &ShellRunParams {
            command: "printf hello".to_string(),
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

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let pending = decode_shell_outcome(pending_task.await.unwrap());
    let approval_id = match pending {
        ShellRunOutcome::PendingApproval(PendingApprovalReceipt { approval_id }) => approval_id,
        other => panic!("expected PendingApproval, got {other:?}"),
    };

    (runtime, approval_id)
}

#[tokio::test]
async fn tui_list_pending_approvals_returns_created_approval() {
    let (_tmp, paths) = test_paths();
    let (mut runtime, approval_id) = setup_pending_approval(&paths).await;

    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { client::list_pending_approvals(&socket_path).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = list_task.await.unwrap();
    assert!(result.is_ok(), "list should succeed: {:?}", result.err());
    let approvals = result.unwrap();
    assert!(
        approvals.iter().any(|a| a.approval_id == approval_id),
        "approval_id {approval_id} should be in the list"
    );
    assert_eq!(approvals[0].app_id, "notes");
    assert_eq!(approvals[0].capability, "shell");
}

#[tokio::test]
async fn tui_approve_pending_approval_succeeds() {
    let (_tmp, paths) = test_paths();
    let (mut runtime, approval_id) = setup_pending_approval(&paths).await;

    let approve_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        let aid = approval_id.clone();
        async move { client::approve_pending_approval(&socket_path, aid).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = approve_task.await.unwrap();
    assert!(result.is_ok(), "approve should succeed: {:?}", result.err());

    // Verify the approval is no longer pending
    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { client::list_pending_approvals(&socket_path).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let remaining = list_task.await.unwrap().unwrap();
    assert!(
        remaining.iter().all(|a| a.approval_id != approval_id),
        "approved approval should no longer be pending"
    );
}

#[tokio::test]
async fn tui_reject_pending_approval_succeeds() {
    let (_tmp, paths) = test_paths();
    let (mut runtime, approval_id) = setup_pending_approval(&paths).await;

    let reject_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        let aid = approval_id.clone();
        async move { client::reject_pending_approval(&socket_path, aid).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = reject_task.await.unwrap();
    assert!(result.is_ok(), "reject should succeed: {:?}", result.err());

    // Verify the approval is no longer pending
    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { client::list_pending_approvals(&socket_path).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let remaining = list_task.await.unwrap().unwrap();
    assert!(
        remaining.iter().all(|a| a.approval_id != approval_id),
        "rejected approval should no longer be pending"
    );
}

#[tokio::test]
async fn tui_list_pending_approvals_empty_when_none_exist() {
    let (_tmp, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let list_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { client::list_pending_approvals(&socket_path).await }
    });

    tokio::select! {
        result = runtime.run_once() => { result.unwrap(); }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("runtime.run_once() timed out");
        }
    }

    let result = list_task.await.unwrap();
    assert!(result.is_ok(), "list should succeed: {:?}", result.err());
    assert!(result.unwrap().is_empty(), "no approvals expected");
}

// ─── Unit tests for App state transitions (no core runtime needed) ───

fn make_item(id: &str, app_id: &str, summary: &str) -> PendingApprovalItem {
    PendingApprovalItem {
        approval_id: id.to_string(),
        app_id: app_id.to_string(),
        capability: "shell".to_string(),
        summary: summary.to_string(),
    }
}

#[test]
fn toggle_view_switches_between_ping_and_approvals() {
    let mut app = App::new();
    assert_eq!(app.current_view, View::Approvals);

    app.toggle_view();
    assert_eq!(app.current_view, View::Ping);

    app.toggle_view();
    assert_eq!(app.current_view, View::Approvals);
}

#[test]
fn move_selection_down_bounded_by_list_length() {
    let mut app = App::new();
    app.set_approvals(vec![
        make_item("a1", "notes", "first"),
        make_item("a2", "notes", "second"),
        make_item("a3", "notes", "third"),
    ]);

    assert_eq!(app.selected_index, 0);

    app.move_selection_down();
    assert_eq!(app.selected_index, 1);

    app.move_selection_down();
    assert_eq!(app.selected_index, 2);

    // Should not go past the last item.
    app.move_selection_down();
    assert_eq!(app.selected_index, 2);
}

#[test]
fn move_selection_up_bounded_at_zero() {
    let mut app = App::new();
    app.set_approvals(vec![
        make_item("a1", "notes", "first"),
        make_item("a2", "notes", "second"),
    ]);
    app.selected_index = 1;

    app.move_selection_up();
    assert_eq!(app.selected_index, 0);

    // Should not go below 0.
    app.move_selection_up();
    assert_eq!(app.selected_index, 0);
}

#[test]
fn move_selection_noop_on_empty_list() {
    let mut app = App::new();
    assert!(app.approvals.is_empty());

    app.move_selection_down();
    assert_eq!(app.selected_index, 0);

    app.move_selection_up();
    assert_eq!(app.selected_index, 0);
}

#[test]
fn set_approvals_clamps_selected_index() {
    let mut app = App::new();
    app.set_approvals(vec![
        make_item("a1", "notes", "first"),
        make_item("a2", "notes", "second"),
        make_item("a3", "notes", "third"),
    ]);
    app.selected_index = 2;

    // Replace with a shorter list — index should clamp.
    app.set_approvals(vec![make_item("b1", "notes", "only")]);
    assert_eq!(app.selected_index, 0);
    assert!(matches!(app.approvals_status, ApprovalsStatus::Loaded));
}

#[test]
fn set_approvals_with_empty_list_clamps_to_zero() {
    let mut app = App::new();
    app.set_approvals(vec![make_item("a1", "notes", "first")]);
    app.selected_index = 0;

    app.set_approvals(vec![]);
    assert_eq!(app.selected_index, 0);
    assert_eq!(app.approvals.len(), 0);
}

#[test]
fn apply_approval_result_success_sets_loading() {
    let mut app = App::new();
    app.approvals_status = ApprovalsStatus::Loaded;

    app.apply_approval_result(Ok(()));
    assert!(matches!(app.approvals_status, ApprovalsStatus::Loading));
}

#[test]
fn apply_approval_result_failure_stores_error() {
    let mut app = App::new();
    app.approvals_status = ApprovalsStatus::Loaded;

    app.apply_approval_result(Err("denied".to_string()));
    assert!(matches!(app.approvals_status, ApprovalsStatus::Error(ref msg) if msg == "denied"),);
}

#[test]
fn selected_approval_returns_none_for_empty_list() {
    let app = App::new();
    assert!(app.selected_approval().is_none());
}

#[test]
fn selected_approval_returns_correct_item() {
    let mut app = App::new();
    app.set_approvals(vec![
        make_item("a1", "notes", "first"),
        make_item("a2", "notes", "second"),
    ]);

    let selected = app.selected_approval().unwrap();
    assert_eq!(selected.approval_id, "a1");

    app.move_selection_down();
    let selected = app.selected_approval().unwrap();
    assert_eq!(selected.approval_id, "a2");
}
