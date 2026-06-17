use kunkka_core::capability::fs::{ReadFileParams, ReadFileResult};
use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_worker_sdk::{call_capability, AppId};
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

fn write_manifest_with_fs(paths: &KunkkaPaths, allowed_dir: &std::path::Path) {
    let apps_dir = paths.config_dir.join("apps");
    std::fs::create_dir_all(&apps_dir).unwrap();
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
                    "fs": {{
                        "paths": ["{}/"]
                    }}
                }}
            }}"#,
            allowed_dir.display()
        ),
    )
    .unwrap();
}

#[tokio::test]
async fn call_capability_reads_file_from_core_runtime() {
    let (_root, paths) = test_paths();
    let workspace = tempdir().unwrap();
    let file_path = workspace.path().join("note.txt");
    std::fs::write(&file_path, "hello from runtime").unwrap();
    write_manifest_with_fs(&paths, workspace.path());

    let mut runtime = prepare_core_runtime(&paths).await.unwrap();
    let params = postcard::to_stdvec(&ReadFileParams {
        path: file_path.to_string_lossy().into_owned(),
    })
    .unwrap();

    let client = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            call_capability(
                &socket_path,
                &AppId::new("notes"),
                "fs",
                "read_file",
                params,
            )
            .await
            .unwrap()
        }
    });

    runtime.run_once().await.unwrap();
    let response = client.await.unwrap();
    let result = response.result.unwrap();
    let read_result: ReadFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(read_result.content, "hello from runtime");
}
