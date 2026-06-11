use kunkka_native_host::path::{resolve_core_socket_path_from_env, CoreSocketPathEnv};
use std::path::PathBuf;

#[test]
fn resolves_socket_under_absolute_xdg_runtime_dir() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
    };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(path, PathBuf::from("/run/user/1000/kunkka/core.sock"));
}

#[test]
fn ignores_relative_xdg_runtime_dir_and_uses_tmp_fallback() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: Some(PathBuf::from("relative-runtime")),
    };
    let uid = unsafe { libc::geteuid() as u32 };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(
        path,
        PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
    );
}

#[test]
fn uses_tmp_fallback_when_xdg_runtime_dir_is_missing() {
    let env = CoreSocketPathEnv {
        xdg_runtime_dir: None,
    };
    let uid = unsafe { libc::geteuid() as u32 };

    let path = resolve_core_socket_path_from_env(&env);

    assert_eq!(
        path,
        PathBuf::from(format!("/tmp/kunkka-runtime-{uid}/core.sock"))
    );
}
