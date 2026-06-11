use kunkka_core::xdg::{KunkkaPaths, PathEnv};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn path_mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn resolves_absolute_xdg_directories() {
    let home = tempdir().unwrap();
    let config = tempdir().unwrap();
    let data = tempdir().unwrap();
    let state = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let runtime = tempdir().unwrap();

    let env = PathEnv {
        home: Some(home.path().to_path_buf()),
        xdg_config_home: Some(config.path().to_path_buf()),
        xdg_data_home: Some(data.path().to_path_buf()),
        xdg_state_home: Some(state.path().to_path_buf()),
        xdg_cache_home: Some(cache.path().to_path_buf()),
        xdg_runtime_dir: Some(runtime.path().to_path_buf()),
    };

    let paths = KunkkaPaths::resolve_from_env(&env).unwrap();

    assert_eq!(paths.config_dir, config.path().join("kunkka"));
    assert_eq!(paths.data_dir, data.path().join("kunkka"));
    assert_eq!(paths.state_dir, state.path().join("kunkka"));
    assert_eq!(paths.cache_dir, cache.path().join("kunkka"));
    assert_eq!(paths.runtime_dir, runtime.path().join("kunkka"));
    assert_eq!(paths.database_path, data.path().join("kunkka/kunkka.db"));
    assert_eq!(paths.log_dir, state.path().join("kunkka/logs"));
    assert_eq!(paths.socket_path, runtime.path().join("kunkka/core.sock"));
}

#[test]
fn falls_back_to_home_based_xdg_directories() {
    let home = tempdir().unwrap();

    let env = PathEnv {
        home: Some(home.path().to_path_buf()),
        ..PathEnv::default()
    };

    let paths = KunkkaPaths::resolve_from_env(&env).unwrap();

    assert_eq!(paths.config_dir, home.path().join(".config/kunkka"));
    assert_eq!(paths.data_dir, home.path().join(".local/share/kunkka"));
    assert_eq!(paths.state_dir, home.path().join(".local/state/kunkka"));
    assert_eq!(paths.cache_dir, home.path().join(".cache/kunkka"));
    assert_eq!(
        paths.database_path,
        home.path().join(".local/share/kunkka/kunkka.db")
    );
    assert_eq!(paths.log_dir, home.path().join(".local/state/kunkka/logs"));
}

#[test]
fn ignores_relative_xdg_values() {
    let home = tempdir().unwrap();

    let env = PathEnv {
        home: Some(home.path().to_path_buf()),
        xdg_config_home: Some(PathBuf::from("relative-config")),
        xdg_data_home: Some(PathBuf::from("relative-data")),
        xdg_state_home: Some(PathBuf::from("relative-state")),
        xdg_cache_home: Some(PathBuf::from("relative-cache")),
        xdg_runtime_dir: Some(PathBuf::from("relative-runtime")),
    };

    let paths = KunkkaPaths::resolve_from_env(&env).unwrap();

    assert_eq!(paths.config_dir, home.path().join(".config/kunkka"));
    assert_eq!(paths.data_dir, home.path().join(".local/share/kunkka"));
    assert_eq!(paths.state_dir, home.path().join(".local/state/kunkka"));
    assert_eq!(paths.cache_dir, home.path().join(".cache/kunkka"));
}

#[test]
fn falls_back_to_secure_tmp_runtime_dir() {
    let home = tempdir().unwrap();

    let env = PathEnv {
        home: Some(home.path().to_path_buf()),
        xdg_runtime_dir: None,
        ..PathEnv::default()
    };

    let paths = KunkkaPaths::resolve_from_env(&env).unwrap();

    let uid = unsafe {
        // SAFETY: geteuid has no preconditions and does not dereference pointers.
        libc::geteuid() as u32
    };

    assert_eq!(
        paths.runtime_dir,
        PathBuf::from(format!("/tmp/kunkka-runtime-{uid}"))
    );
    assert_eq!(paths.socket_path, paths.runtime_dir.join("core.sock"));
}

#[test]
fn ensure_dirs_creates_private_directories() {
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

    paths.ensure_dirs().unwrap();

    assert_eq!(path_mode(&paths.config_dir), 0o700);
    assert_eq!(path_mode(&paths.data_dir), 0o700);
    assert_eq!(path_mode(&paths.state_dir), 0o700);
    assert_eq!(path_mode(&paths.cache_dir), 0o700);
    assert_eq!(path_mode(&paths.runtime_dir), 0o700);
    assert_eq!(path_mode(&paths.log_dir), 0o700);
}
