pub mod cli;
pub mod client;
pub mod error;
pub mod output;

use std::env;
use std::path::PathBuf;

/// Resolve the Kunkka core socket path using XDG conventions.
///
/// This mirrors the logic in `kunkka-core::xdg::KunkkaPaths` but is kept
/// minimal so that `kunkka-cli` does not depend on `kunkka-core` at runtime.
pub fn resolve_socket_path() -> Result<PathBuf, error::CliError> {
    let home = env_path("HOME")
        .ok_or_else(|| error::CliError::CoreUnavailable("HOME is not set".to_string()))?;

    let runtime_dir = env_path("XDG_RUNTIME_DIR")
        .map(|path| path.join("kunkka"))
        .unwrap_or_else(|| {
            PathBuf::from(format!("/tmp/kunkka-runtime-{}", effective_uid()))
        });

    Ok(runtime_dir.join("core.sock"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() as u32 }
}
