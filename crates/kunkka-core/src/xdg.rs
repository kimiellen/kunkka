use crate::{CoreError, Result};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathEnv {
    pub home: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub xdg_state_home: Option<PathBuf>,
    pub xdg_cache_home: Option<PathBuf>,
    pub xdg_runtime_dir: Option<PathBuf>,
}

impl PathEnv {
    pub fn from_process() -> Self {
        Self {
            home: env_path("HOME"),
            xdg_config_home: env_path("XDG_CONFIG_HOME"),
            xdg_data_home: env_path("XDG_DATA_HOME"),
            xdg_state_home: env_path("XDG_STATE_HOME"),
            xdg_cache_home: env_path("XDG_CACHE_HOME"),
            xdg_runtime_dir: env_path("XDG_RUNTIME_DIR"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KunkkaPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub database_path: PathBuf,
    pub log_dir: PathBuf,
    pub socket_path: PathBuf,
}

impl KunkkaPaths {
    pub fn resolve() -> Result<Self> {
        Self::resolve_from_env(&PathEnv::from_process())
    }

    pub fn resolve_from_env(path_env: &PathEnv) -> Result<Self> {
        let home = absolute_env_path(&path_env.home).ok_or(CoreError::MissingHome)?;

        let config_base =
            absolute_env_path(&path_env.xdg_config_home).unwrap_or_else(|| home.join(".config"));
        let data_base =
            absolute_env_path(&path_env.xdg_data_home).unwrap_or_else(|| home.join(".local/share"));
        let state_base = absolute_env_path(&path_env.xdg_state_home)
            .unwrap_or_else(|| home.join(".local/state"));
        let cache_base =
            absolute_env_path(&path_env.xdg_cache_home).unwrap_or_else(|| home.join(".cache"));

        let runtime_dir = absolute_env_path(&path_env.xdg_runtime_dir)
            .map(|path| path.join("kunkka"))
            .unwrap_or_else(runtime_fallback_dir);

        let config_dir = config_base.join("kunkka");
        let data_dir = data_base.join("kunkka");
        let state_dir = state_base.join("kunkka");
        let cache_dir = cache_base.join("kunkka");
        let database_path = data_dir.join("kunkka.db");
        let log_dir = state_dir.join("logs");
        let socket_path = runtime_dir.join("core.sock");

        Ok(Self {
            config_dir,
            data_dir,
            state_dir,
            cache_dir,
            runtime_dir,
            database_path,
            log_dir,
            socket_path,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        ensure_private_dir(&self.config_dir)?;
        ensure_private_dir(&self.data_dir)?;
        ensure_private_dir(&self.state_dir)?;
        ensure_private_dir(&self.cache_dir)?;
        ensure_private_dir(&self.runtime_dir)?;
        ensure_private_dir(&self.log_dir)?;
        Ok(())
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn absolute_env_path(path: &Option<PathBuf>) -> Option<PathBuf> {
    path.as_ref().filter(|path| path.is_absolute()).cloned()
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

fn ensure_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
