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
