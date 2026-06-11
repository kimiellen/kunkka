use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreSocketPathEnv {
    pub xdg_runtime_dir: Option<PathBuf>,
}

impl CoreSocketPathEnv {
    pub fn from_process() -> Self {
        Self {
            xdg_runtime_dir: env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        }
    }
}

pub fn resolve_core_socket_path() -> PathBuf {
    resolve_core_socket_path_from_env(&CoreSocketPathEnv::from_process())
}

pub fn resolve_core_socket_path_from_env(env: &CoreSocketPathEnv) -> PathBuf {
    let runtime_dir = env
        .xdg_runtime_dir
        .as_ref()
        .filter(|path| path.is_absolute())
        .map(|path| path.join("kunkka"))
        .unwrap_or_else(runtime_fallback_dir);

    runtime_dir.join("core.sock")
}

fn runtime_fallback_dir() -> PathBuf {
    PathBuf::from(format!("/tmp/kunkka-runtime-{}", effective_uid()))
}

fn effective_uid() -> u32 {
    unsafe {
        // SAFETY: geteuid has no preconditions and does not dereference pointers.
        libc::geteuid() as u32
    }
}
