use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
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

fn request() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("worker-1"),
        app_id: AppId::new("example-app"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: Some("Search notes".to_string()),
        }],
    }
}

#[tokio::test]
async fn prepare_core_runtime_creates_dirs_binds_socket_and_starts_empty() {
    let (_root, paths) = test_paths();

    let runtime = prepare_core_runtime(&paths).await.unwrap();

    assert!(paths.config_dir.exists());
    assert!(paths.data_dir.exists());
    assert!(paths.state_dir.exists());
    assert!(paths.cache_dir.exists());
    assert!(paths.runtime_dir.exists());
    assert!(paths.log_dir.exists());
    assert!(paths.socket_path.exists());
    assert!(runtime.registry().is_empty());
}

#[tokio::test]
async fn run_once_accepts_worker_registration_and_updates_registry() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();

        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("worker-1"))
                .await
                .unwrap();

            client.register(request()).await.unwrap()
        }
    });

    runtime.run_once().await.unwrap();

    let response = register_task.await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.worker_id.as_str(), "worker-1");
    assert!(runtime.registry().get(&WorkerId::new("worker-1")).is_some());
}
