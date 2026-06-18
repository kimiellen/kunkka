use kunkka_core::app_manifest::AppManifest;
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

#[test]
fn test_http_capability_config_loaded() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "test.json",
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            },
            "capabilities": {
                "http": {
                    "domains": ["api.github.com", "hooks.slack.com"]
                }
            }
        }"#,
    );

    let manifest = AppManifest::load_file(paths.config_dir.join("apps/test.json")).unwrap();
    let http = manifest.capabilities.http.unwrap();
    assert_eq!(http.domains, vec!["api.github.com", "hooks.slack.com"]);
}

#[test]
fn test_http_capability_config_empty() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "test.json",
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            }
        }"#,
    );

    let manifest = AppManifest::load_file(paths.config_dir.join("apps/test.json")).unwrap();
    assert!(manifest.capabilities.http.is_none());
}

#[test]
fn test_http_blank_domain_rejected() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "test.json",
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            },
            "capabilities": {
                "http": {
                    "domains": [""]
                }
            }
        }"#,
    );

    let result = AppManifest::load_file(paths.config_dir.join("apps/test.json"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        CoreError::ManifestInvalid(msg) if msg.contains("blank domain")
    ));
}

#[test]
fn test_http_domain_with_whitespace_rejected() {
    let (_root, paths) = test_paths();
    write_manifest(
        &paths,
        "test.json",
        r#"{
            "app_id": "test",
            "worker": {
                "program": "/usr/bin/test",
                "args": []
            },
            "capabilities": {
                "http": {
                    "domains": [" api.github.com "]
                }
            }
        }"#,
    );

    let result = AppManifest::load_file(paths.config_dir.join("apps/test.json"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        CoreError::ManifestInvalid(msg) if msg.contains("leading or trailing whitespace")
    ));
}
