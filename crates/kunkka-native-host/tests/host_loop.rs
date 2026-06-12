use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::host::run_native_host;
use kunkka_native_host::native_messaging::{
    read_native_message, write_native_message, MAX_NATIVE_MESSAGE_LEN,
};
use serde_json::json;
use std::io::Cursor;
use tempfile::{tempdir, TempDir};

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

#[tokio::test]
async fn host_loop_reads_native_message_and_writes_response() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let runtime_task = tokio::spawn(async move { runtime.run_once().await.unwrap() });

    let mut input_bytes = Vec::new();
    write_native_message(&mut input_bytes, &json!({"id":"req-1","command":"ping"})).unwrap();

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    drop(session);
    runtime_task.await.unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({"id":"req-1","ok":true,"result":{"type":"pong"}})
    );
}

#[tokio::test]
async fn host_loop_returns_invalid_request_with_null_id_for_missing_id() {
    let (_root, paths) = test_paths();
    let mut input_bytes = Vec::new();
    write_native_message(&mut input_bytes, &json!({"command":"ping"})).unwrap();

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": "missing request id"
            }
        })
    );
}

#[tokio::test]
async fn host_loop_returns_invalid_request_for_invalid_length_prefix() {
    let (_root, paths) = test_paths();
    let mut input_bytes = Vec::new();
    input_bytes.extend_from_slice(&((MAX_NATIVE_MESSAGE_LEN as u32) + 1).to_le_bytes());

    let mut input = Cursor::new(input_bytes);
    let mut output = Vec::new();
    let mut session = NativeHostSession::new(paths.socket_path.clone());

    run_native_host(&mut input, &mut output, &mut session)
        .await
        .unwrap();

    let mut output_reader = Cursor::new(output);
    let response_bytes = read_native_message(&mut output_reader).unwrap().unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();

    assert_eq!(
        response,
        json!({
            "id": null,
            "ok": false,
            "error": {
                "code": "invalid_request",
                "message": format!("native message too large: {} bytes", MAX_NATIVE_MESSAGE_LEN + 1)
            }
        })
    );
}
