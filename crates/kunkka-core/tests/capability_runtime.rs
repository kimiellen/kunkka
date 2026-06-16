use kunkka_core::capability::fs::{
    ListDirParams, ListDirResult, ReadFileParams, ReadFileResult, WriteFileParams, WriteFileResult,
};
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

fn write_manifest_with_fs(paths: &KunkkaPaths, fs_paths: Vec<String>) {
    use std::fs;
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    let paths_json: Vec<String> = fs_paths.into_iter().map(|p| format!("\"{p}\"")).collect();
    let body = format!(
        r#"{{
            "app_id": "notes",
            "worker": {{
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"]
            }},
            "permissions": {{
                "frontend_dispatch": {{
                    "allowed_methods": ["search"]
                }}
            }},
            "capabilities": {{
                "fs": {{
                    "paths": [{}]
                }}
            }}
        }}"#,
        paths_json.join(", ")
    );
    fs::write(apps_dir.join("notes.json"), body).unwrap();
}

fn write_manifest_without_capabilities(paths: &KunkkaPaths) {
    use std::fs;
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(
        apps_dir.join("notes.json"),
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
    )
    .unwrap();
}

fn capability_frame(
    request_id: u128,
    app_id: &str,
    capability: &str,
    method: &str,
    params: &[u8],
) -> Frame {
    let payload = encode_capability_request(&CapabilityRequest {
        app_id: app_id.to_string(),
        capability: capability.to_string(),
        method: method.to_string(),
        params: params.to_vec(),
    })
    .unwrap();

    Frame::Request {
        request_id: RequestId(request_id),
        session_id: SessionId(1),
        source: EndpointId::new("test-client"),
        target: EndpointId::new("core"),
        payload,
        metadata: FrameMetadata::new(),
    }
}

fn extract_capability_response(frame: Frame) -> Result<Vec<u8>, CapabilityError> {
    let Frame::Response { payload, .. } = frame else {
        panic!("expected response frame");
    };
    decode_capability_response(&payload).unwrap().result
}

#[tokio::test]
async fn capability_read_file_returns_content() {
    let (_root, paths) = test_paths();
    let test_dir = _root.path().join("workspace");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.txt"), "hello kunkka").unwrap();
    write_manifest_with_fs(&paths, vec![format!("{}/", test_dir.display())]);

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        1,
        "notes",
        "fs",
        "read_file",
        &postcard::to_stdvec(&ReadFileParams {
            path: test_dir.join("hello.txt").to_string_lossy().into_owned(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let result = extract_capability_response(response_frame).unwrap();
    let read_result: ReadFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(read_result.content, "hello kunkka");
}

#[tokio::test]
async fn capability_write_file_creates_file() {
    let (_root, paths) = test_paths();
    let test_dir = _root.path().join("workspace");
    std::fs::create_dir_all(&test_dir).unwrap();
    write_manifest_with_fs(&paths, vec![format!("{}/", test_dir.display())]);

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        2,
        "notes",
        "fs",
        "write_file",
        &postcard::to_stdvec(&WriteFileParams {
            path: test_dir.join("output.txt").to_string_lossy().into_owned(),
            content: "written by capability".to_string(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let result = extract_capability_response(response_frame).unwrap();
    let write_result: WriteFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(write_result.bytes_written, 21);
    assert_eq!(
        std::fs::read_to_string(test_dir.join("output.txt")).unwrap(),
        "written by capability"
    );
}

#[tokio::test]
async fn capability_list_dir_returns_entries() {
    let (_root, paths) = test_paths();
    let test_dir = _root.path().join("workspace");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("a.txt"), "aaa").unwrap();
    std::fs::write(test_dir.join("b.txt"), "bbb").unwrap();
    std::fs::create_dir(test_dir.join("subdir")).unwrap();
    write_manifest_with_fs(&paths, vec![format!("{}/", test_dir.display())]);

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        3,
        "notes",
        "fs",
        "list_dir",
        &postcard::to_stdvec(&ListDirParams {
            path: test_dir.to_string_lossy().into_owned(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let result = extract_capability_response(response_frame).unwrap();
    let list_result: ListDirResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(list_result.entries.len(), 3);
    let names: Vec<&str> = list_result
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
    assert!(names.contains(&"subdir"));
}

#[tokio::test]
async fn capability_denies_path_not_in_whitelist() {
    let (_root, paths) = test_paths();
    let test_dir = _root.path().join("workspace");
    std::fs::create_dir_all(&test_dir).unwrap();
    write_manifest_with_fs(&paths, vec![format!("{}/", test_dir.display())]);

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        4,
        "notes",
        "fs",
        "read_file",
        &postcard::to_stdvec(&ReadFileParams {
            path: "/etc/passwd".to_string(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let err = extract_capability_response(response_frame).unwrap_err();
    assert_eq!(err.code, "permission_denied");
    assert!(err.message.contains("/etc/passwd"));
}

#[tokio::test]
async fn capability_denies_when_no_capabilities_config() {
    let (_root, paths) = test_paths();
    let test_dir = _root.path().join("workspace");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.txt"), "hello").unwrap();
    write_manifest_without_capabilities(&paths);

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        5,
        "notes",
        "fs",
        "read_file",
        &postcard::to_stdvec(&ReadFileParams {
            path: test_dir.join("hello.txt").to_string_lossy().into_owned(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let err = extract_capability_response(response_frame).unwrap_err();
    assert_eq!(err.code, "permission_denied");
    assert!(err.message.contains("no fs capability"));
}

#[tokio::test]
async fn capability_returns_app_not_found_for_unknown_app() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let frame = capability_frame(
        6,
        "unknown-app",
        "fs",
        "read_file",
        &postcard::to_stdvec(&ReadFileParams {
            path: "/tmp/x".to_string(),
        })
        .unwrap(),
    );

    let client_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut connection = IpcConnection::connect(&socket_path).await.unwrap();
            connection.send_frame(&frame).await.unwrap();
            connection.recv_frame().await.unwrap().unwrap()
        }
    });

    wait_for(runtime.run_once()).await.unwrap();
    let response_frame = wait_for(client_task).await.unwrap();
    let err = extract_capability_response(response_frame).unwrap_err();
    assert_eq!(err.code, "app_not_found");
    assert!(err.message.contains("unknown-app"));
}
