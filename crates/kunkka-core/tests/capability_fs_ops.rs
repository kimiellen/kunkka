use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, FsCapabilityConfig, WorkerCommand,
};
use kunkka_core::capability::fs::{
    handle_fs_request, ListDirParams, ListDirResult, ReadFileParams, ReadFileResult,
    WriteFileParams, WriteFileResult,
};
use kunkka_core::capability::CapabilityError;
use kunkka_worker_sdk::AppId;
use std::fs;

fn manifest_with_dir(dir: &std::path::Path) -> AppManifest {
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
                paths: vec![format!("{}/", dir.display())],
            }),
            shell: None,
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[tokio::test]
async fn read_file_returns_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let manifest = manifest_with_dir(dir.path());

    let params = postcard::to_stdvec(&ReadFileParams {
        path: format!("{}/hello.txt", dir.path().display()),
    })
    .unwrap();
    let result = handle_fs_request(&manifest, "read_file", &params)
        .await
        .unwrap();
    let parsed: ReadFileResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(parsed.content, "hello world");
}

#[tokio::test]
async fn write_file_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());
    let target = format!("{}/output.txt", dir.path().display());

    let params = postcard::to_stdvec(&WriteFileParams {
        path: target.clone(),
        content: "written data".to_string(),
    })
    .unwrap();
    let result = handle_fs_request(&manifest, "write_file", &params)
        .await
        .unwrap();
    let parsed: WriteFileResult = postcard::from_bytes(&result).unwrap();
    assert!(parsed.bytes_written > 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), "written data");
}

#[tokio::test]
async fn list_dir_returns_entries() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::write(dir.path().join("b.txt"), "b").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    let manifest = manifest_with_dir(dir.path());

    let params = postcard::to_stdvec(&ListDirParams {
        path: format!("{}", dir.path().display()),
    })
    .unwrap();
    let result = handle_fs_request(&manifest, "list_dir", &params)
        .await
        .unwrap();
    let parsed: ListDirResult = postcard::from_bytes(&result).unwrap();
    assert_eq!(parsed.entries.len(), 3);
    let names: Vec<&str> = parsed.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
    assert!(names.contains(&"sub"));
    let sub = parsed.entries.iter().find(|e| e.name == "sub").unwrap();
    assert_eq!(sub.entry_type, "dir");
}

#[tokio::test]
async fn read_file_denied_outside_whitelist() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());

    let params = postcard::to_stdvec(&ReadFileParams {
        path: "/etc/passwd".to_string(),
    })
    .unwrap();
    let result = handle_fs_request(&manifest, "read_file", &params).await;
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "permission_denied"));
}

#[tokio::test]
async fn unknown_method_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest_with_dir(dir.path());

    let result = handle_fs_request(&manifest, "no_such_method", &[]).await;
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "unknown_method"));
}

#[tokio::test]
async fn read_file_non_utf8_returns_not_utf8() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("binary.bin"), [0xFF, 0xFE, 0x00, 0x01]).unwrap();
    let manifest = manifest_with_dir(dir.path());

    let params = postcard::to_stdvec(&ReadFileParams {
        path: format!("{}/binary.bin", dir.path().display()),
    })
    .unwrap();
    let result = handle_fs_request(&manifest, "read_file", &params).await;
    assert!(matches!(result, Err(CapabilityError { code, .. }) if code == "not_utf8"));
}
