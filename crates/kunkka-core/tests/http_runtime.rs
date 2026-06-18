use kunkka_core::capability::http::HttpRequestParams;
use kunkka_core::capability::{
    decode_capability_response, encode_capability_request, CapabilityError, CapabilityRequest,
};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcConnection, RequestId, SessionId};
use std::future::Future;
use tempfile::{tempdir, TempDir};
use tokio::time::{timeout, Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

async fn wait_for<T>(future: impl Future<Output = T>) -> T {
    timeout(TEST_TIMEOUT, future)
        .await
        .expect("test operation timed out")
}

fn test_paths() -> (TempDir, KunkkaPaths) {
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

fn write_manifest_with_http(paths: &KunkkaPaths, domains: &[&str]) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();

    let domains_json = domains
        .iter()
        .map(|d| format!("\"{d}\""))
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
                    "http": {{
                        "domains": [{domains_json}]
                    }}
                }}
            }}"#,
        ),
    )
    .unwrap();
}

fn capability_frame(request_id: u128, params: &HttpRequestParams) -> Frame {
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: "notes".to_string(),
        capability: "http".to_string(),
        method: "request".to_string(),
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

async fn connect_and_send(socket_path: &std::path::Path, frame: Frame) -> Frame {
    let mut connection = IpcConnection::connect(socket_path).await.unwrap();
    connection.send_frame(&frame).await.unwrap();
    connection.recv_frame().await.unwrap().unwrap()
}

fn extract_capability_result(frame: Frame) -> Result<Vec<u8>, CapabilityError> {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    decode_capability_response(&payload).unwrap().result
}

#[tokio::test]
async fn test_http_domain_not_allowed() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        1,
        &HttpRequestParams {
            method: "GET".to_string(),
            url: "https://evil.com/data".to_string(),
            headers: vec![],
            body: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { connect_and_send(&socket_path, frame).await }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = extract_capability_result(wait_for(client_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "permission_denied"));
}

#[tokio::test]
async fn test_http_invalid_url() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        2,
        &HttpRequestParams {
            method: "GET".to_string(),
            url: "not a url".to_string(),
            headers: vec![],
            body: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { connect_and_send(&socket_path, frame).await }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = extract_capability_result(wait_for(client_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "invalid_params"));
}

#[tokio::test]
async fn test_http_scheme_not_allowed() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        3,
        &HttpRequestParams {
            method: "GET".to_string(),
            url: "ftp://api.github.com/file".to_string(),
            headers: vec![],
            body: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { connect_and_send(&socket_path, frame).await }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = extract_capability_result(wait_for(client_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "scheme_not_allowed"));
}

#[tokio::test]
async fn test_http_invalid_method() {
    let (_root, paths) = test_paths();
    write_manifest_with_http(&paths, &["api.github.com"]);
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        4,
        &HttpRequestParams {
            method: "TRACE".to_string(),
            url: "https://api.github.com".to_string(),
            headers: vec![],
            body: None,
        },
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move { connect_and_send(&socket_path, frame).await }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let result = extract_capability_result(wait_for(client_task).await.unwrap());
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "invalid_params"));
}
