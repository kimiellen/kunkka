use kunkka_core::prepare_core_runtime;
use kunkka_core::xdg::KunkkaPaths;
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerClient, WorkerId};
use std::time::Duration;
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

fn request(worker_id: &str, app_id: &str) -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new(worker_id),
        app_id: AppId::new(app_id),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

#[tokio::test]
async fn runtime_hands_registered_worker_connection_to_worker_manager() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    let register_task = tokio::spawn({
        let socket_path = paths.socket_path.clone();
        async move {
            let mut client = WorkerClient::connect(&socket_path, WorkerId::new("notes"))
                .await
                .unwrap();
            client.register(request("notes", "notes")).await.unwrap()
        }
    });

    tokio::time::timeout(Duration::from_secs(2), runtime.run_once())
        .await
        .unwrap()
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), register_task)
        .await
        .unwrap()
        .unwrap();

    assert!(response.accepted);
    assert!(runtime
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_some());
    assert!(runtime.worker_manager().is_active(&AppId::new("notes")));
}

#[tokio::test]
async fn duplicate_app_registration_replaces_runtime_active_worker() {
    let (_root, paths) = test_paths();
    let mut runtime = prepare_core_runtime(&paths).await.unwrap();

    for worker_id in ["worker-1", "worker-2"] {
        let register_task = tokio::spawn({
            let socket_path = paths.socket_path.clone();
            async move {
                let mut client = WorkerClient::connect(&socket_path, WorkerId::new(worker_id))
                    .await
                    .unwrap();
                client.register(request(worker_id, "notes")).await.unwrap()
            }
        });
        tokio::time::timeout(Duration::from_secs(2), runtime.run_once())
            .await
            .unwrap()
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), register_task)
                .await
                .unwrap()
                .unwrap()
                .accepted
        );
    }

    assert_eq!(runtime.registry().len(), 1);
    let registered = runtime
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .unwrap();
    assert_eq!(registered.worker_id.as_str(), "worker-2");
    assert_eq!(runtime.worker_manager().active_worker_count(), 1);
}
