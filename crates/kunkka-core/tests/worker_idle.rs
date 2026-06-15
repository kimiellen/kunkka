use kunkka_core::worker_dispatch::WorkerManager;
use kunkka_ipc::{IpcConnection, IpcListener};
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerId};
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let socket_path = root.path().join("worker.sock");
    (root, socket_path)
}

fn registration() -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new("notes"),
        app_id: AppId::new("notes"),
        capabilities: vec![WorkerCapability {
            name: "notes.search".to_string(),
            description: None,
        }],
    }
}

#[tokio::test]
async fn reap_idle_workers_removes_expired_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 1);
    assert!(manager.is_active(&AppId::new("notes")));

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    manager.reap_idle_workers();

    assert!(!manager.is_active(&AppId::new("notes")));
    assert_eq!(manager.active_worker_count(), 0);
    assert!(manager
        .registry()
        .get_by_app_id(&AppId::new("notes"))
        .is_none());
    worker_task.await.unwrap();
}

#[tokio::test]
async fn reap_idle_workers_keeps_recent_active_worker() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();
    let worker_task = tokio::spawn({
        let socket_path = socket_path.clone();
        async move {
            let _connection = IpcConnection::connect(&socket_path).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let core_connection = listener.accept().await.unwrap();

    let mut manager = WorkerManager::new_empty();
    manager.register_active_for_test(registration(), core_connection, 60_000);
    manager.reap_idle_workers();

    assert!(manager.is_active(&AppId::new("notes")));
    worker_task.await.unwrap();
}
