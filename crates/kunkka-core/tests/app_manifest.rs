use kunkka_core::app_manifest::{
    AppManifest, AppRegistry, DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_STARTUP_TIMEOUT_MS,
};
use kunkka_core::xdg::KunkkaPaths;
use kunkka_core::CoreError;
use std::fs;
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

fn write_manifest(paths: &KunkkaPaths, name: &str, body: &str) {
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(apps_dir.join(name), body).unwrap();
}

fn manifest_invalid_message(err: CoreError) -> String {
    match err {
        CoreError::ManifestInvalid(message) => message,
        other => panic!("expected ManifestInvalid, got {other:?}"),
    }
}

#[test]
fn loads_app_manifest_from_xdg_config_apps_dir() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": ["--serve"],
                "env": { "NOTES_ENV": "local" },
                "cwd": "/home/example"
            },
            "idle_timeout_ms": 1234,
            "startup_timeout_ms": 5678
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert_eq!(manifest.app_id.as_str(), "notes");
    assert_eq!(manifest.worker.program, "/usr/bin/notes-worker");
    assert_eq!(manifest.worker.args, vec!["--serve"]);
    assert_eq!(manifest.worker.env.get("NOTES_ENV").unwrap(), "local");
    assert_eq!(
        manifest.worker.cwd.as_deref(),
        Some(std::path::Path::new("/home/example"))
    );
    assert_eq!(manifest.idle_timeout_ms, 1234);
    assert_eq!(manifest.startup_timeout_ms, 5678);
}

#[test]
fn uses_default_timeouts_when_manifest_omits_timeout_fields() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let registry = AppRegistry::load(&paths).unwrap();
    let manifest = registry.get("notes").unwrap();

    assert_eq!(manifest.idle_timeout_ms, DEFAULT_IDLE_TIMEOUT_MS);
    assert_eq!(manifest.startup_timeout_ms, DEFAULT_STARTUP_TIMEOUT_MS);
}

#[test]
fn missing_apps_dir_loads_empty_registry() {
    let (_root, paths) = test_paths();

    let registry = AppRegistry::load(&paths).unwrap();

    assert!(registry.get("notes").is_none());
    assert!(registry.is_empty());
}

#[test]
fn rejects_manifest_missing_worker_program() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "args": []
            }
        }"#,
    );

    let err = AppRegistry::load(&paths).unwrap_err();

    assert!(matches!(
        err,
        CoreError::ManifestInvalid(message) if message.contains("worker.program")
    ));
}

#[test]
fn rejects_invalid_manifest_json() {
    let (_root, paths) = test_paths();
    write_manifest(&paths, "notes.json", "not json");

    let err = AppRegistry::load(&paths).unwrap_err();

    assert!(matches!(
        err,
        CoreError::ManifestInvalid(message) if message.contains("notes.json")
    ));
}

#[test]
fn loads_manifest_file_directly() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let manifest = AppManifest::load_file(paths.config_dir.join("apps/notes.json")).unwrap();

    assert_eq!(manifest.app_id.as_str(), "notes");
}

#[test]
fn rejects_duplicate_app_ids_across_manifest_files() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "a.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker-a",
                "args": []
            }
        }"#,
    );
    write_manifest(
        &paths,
        "b.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker-b",
                "args": []
            }
        }"#,
    );

    let message = manifest_invalid_message(AppRegistry::load(&paths).unwrap_err());

    assert!(message.contains("duplicate app_id"));
    assert!(message.contains("notes"));
}

#[test]
fn rejects_json_directory_entry() {
    let (_root, paths) = test_paths();
    let apps_dir = paths.config_dir.join("apps");
    fs::create_dir_all(apps_dir.join("not-a-file.json")).unwrap();

    let message = manifest_invalid_message(AppRegistry::load(&paths).unwrap_err());

    assert!(message.contains("not-a-file.json"));
    assert!(message.contains("not a file"));
}

#[test]
fn rejects_blank_app_id() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "   ",
            "worker": {
                "program": "/usr/bin/notes-worker",
                "args": []
            }
        }"#,
    );

    let message = manifest_invalid_message(AppRegistry::load(&paths).unwrap_err());

    assert!(message.contains("app_id"));
}

#[test]
fn rejects_blank_worker_program() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "   ",
                "args": []
            }
        }"#,
    );

    let message = manifest_invalid_message(AppRegistry::load(&paths).unwrap_err());

    assert!(message.contains("worker.program"));
}

#[test]
fn rejects_manifest_missing_worker_args() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "notes.json",
        r#"{
            "app_id": "notes",
            "worker": {
                "program": "/usr/bin/notes-worker"
            }
        }"#,
    );

    let message = manifest_invalid_message(AppRegistry::load(&paths).unwrap_err());

    assert!(message.contains("worker.args"));
}
