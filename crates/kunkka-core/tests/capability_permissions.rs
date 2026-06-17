use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, FsCapabilityConfig, WorkerCommand,
};
use kunkka_core::capability::permissions::check_fs_permission;
use kunkka_worker_sdk::AppId;

fn manifest_with_fs_paths(paths: Vec<&str>) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig {
            fs: Some(FsCapabilityConfig {
                paths: paths.into_iter().map(String::from).collect(),
            }),
            shell: None,
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

fn manifest_without_fs() -> AppManifest {
    AppManifest {
        app_id: AppId::new("test"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[test]
fn allows_exact_file_match() {
    let manifest = manifest_with_fs_paths(vec!["/tmp/export.txt"]);
    assert!(check_fs_permission(&manifest, "/tmp/export.txt").is_ok());
}

#[test]
fn denies_exact_file_mismatch() {
    let manifest = manifest_with_fs_paths(vec!["/tmp/export.txt"]);
    assert!(check_fs_permission(&manifest, "/tmp/other.txt").is_err());
}

#[test]
fn allows_directory_prefix_match() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/notes/todo.txt").is_ok());
    assert!(check_fs_permission(&manifest, "/home/user/notes/sub/item.md").is_ok());
}

#[test]
fn denies_directory_prefix_mismatch() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/other/file.txt").is_err());
}

#[test]
fn denies_when_no_fs_config() {
    let manifest = manifest_without_fs();
    assert!(check_fs_permission(&manifest, "/tmp/file.txt").is_err());
}

#[test]
fn normalizes_dot_segments() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/notes/../notes/todo.txt").is_ok());
}

#[test]
fn normalizes_double_slashes() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user//notes/todo.txt").is_ok());
}

#[test]
fn denies_directory_prefix_sibling() {
    let manifest = manifest_with_fs_paths(vec!["/home/user/notes/"]);
    assert!(check_fs_permission(&manifest, "/home/user/notes-secret/file.txt").is_err());
    assert!(check_fs_permission(&manifest, "/home/user/notesto.txt").is_err());
}
