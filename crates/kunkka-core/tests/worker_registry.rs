use kunkka_core::worker_registry::WorkerRegistry;
use kunkka_worker_sdk::{AppId, RegisterWorkerRequest, WorkerCapability, WorkerId};

fn request(worker_id: &str, app_id: &str, capability: &str) -> RegisterWorkerRequest {
    RegisterWorkerRequest {
        worker_id: WorkerId::new(worker_id),
        app_id: AppId::new(app_id),
        capabilities: vec![WorkerCapability {
            name: capability.to_string(),
            description: None,
        }],
    }
}

#[test]
fn registers_worker_in_memory() {
    let mut registry = WorkerRegistry::new();

    let response = registry.register(request("worker-1", "example-app", "notes.search"));

    assert!(response.accepted);
    assert_eq!(response.worker_id.as_str(), "worker-1");

    let registered = registry.get(&WorkerId::new("worker-1")).unwrap();

    assert_eq!(registry.len(), 1);
    assert_eq!(registered.worker_id.as_str(), "worker-1");
    assert_eq!(registered.app_id.as_str(), "example-app");
    assert_eq!(registered.capabilities[0].name, "notes.search");
}

#[test]
fn duplicate_worker_id_replaces_existing_entry() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "example-app", "notes.search"));
    registry.register(request("worker-1", "example-app", "notes.write"));

    let registered = registry.get(&WorkerId::new("worker-1")).unwrap();

    assert_eq!(registry.len(), 1);
    assert_eq!(registered.capabilities[0].name, "notes.write");
}

#[test]
fn duplicate_app_id_replaces_existing_worker() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "notes", "notes.search"));
    registry.register(request("worker-2", "notes", "notes.write"));

    assert_eq!(registry.len(), 1);
    assert!(registry.get(&WorkerId::new("worker-1")).is_none());

    let registered = registry.get(&WorkerId::new("worker-2")).unwrap();
    assert_eq!(registered.app_id.as_str(), "notes");
    assert_eq!(registered.capabilities[0].name, "notes.write");

    let by_app = registry.get_by_app_id(&AppId::new("notes")).unwrap();
    assert_eq!(by_app.worker_id.as_str(), "worker-2");
}

#[test]
fn same_worker_id_replacing_app_id_removes_old_app_index() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "notes", "notes.search"));
    registry.register(request("worker-1", "tasks", "tasks.search"));

    assert_eq!(registry.len(), 1);
    assert!(registry.get_by_app_id(&AppId::new("notes")).is_none());

    let by_app = registry.get_by_app_id(&AppId::new("tasks")).unwrap();
    assert_eq!(by_app.worker_id.as_str(), "worker-1");
    assert_eq!(by_app.app_id.as_str(), "tasks");
}

#[test]
fn remove_by_worker_id_clears_worker_and_app_indexes() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "notes", "notes.search"));

    let removed = registry.remove(&WorkerId::new("worker-1")).unwrap();

    assert_eq!(removed.app_id.as_str(), "notes");
    assert!(registry.is_empty());
    assert!(registry.get(&WorkerId::new("worker-1")).is_none());
    assert!(registry.get_by_app_id(&AppId::new("notes")).is_none());
}

#[test]
fn remove_by_app_id_clears_app_and_worker_indexes() {
    let mut registry = WorkerRegistry::new();

    registry.register(request("worker-1", "notes", "notes.search"));

    let removed = registry.remove_by_app_id(&AppId::new("notes")).unwrap();

    assert_eq!(removed.worker_id.as_str(), "worker-1");
    assert!(registry.is_empty());
    assert!(registry.get(&WorkerId::new("worker-1")).is_none());
    assert!(registry.get_by_app_id(&AppId::new("notes")).is_none());
}
