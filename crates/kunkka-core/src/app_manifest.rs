use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_worker_sdk::AppId;
use serde::Deserialize;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    pub permissions: AppPermissions,
    pub idle_timeout_ms: u64,
    pub startup_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppPermissions {
    pub frontend_dispatch: FrontendDispatchPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontendDispatchPermissions {
    pub allowed_methods: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawAppManifest {
    app_id: AppId,
    worker: RawWorkerCommand,
    #[serde(default)]
    permissions: Option<RawAppPermissions>,
    #[serde(default = "default_idle_timeout_ms")]
    idle_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    startup_timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct RawWorkerCommand {
    #[serde(default)]
    program: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    cwd: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct RawAppPermissions {
    #[serde(default)]
    frontend_dispatch: Option<RawFrontendDispatchPermissions>,
}

#[derive(Debug, Deserialize)]
struct RawFrontendDispatchPermissions {
    #[serde(default)]
    allowed_methods: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct AppRegistry {
    manifests: BTreeMap<AppId, AppManifest>,
}

impl AppManifest {
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let raw: RawAppManifest = serde_json::from_slice(&bytes)
            .map_err(|err| CoreError::ManifestInvalid(format!("{}: {err}", path.display())))?;
        let manifest = Self::from_raw(raw, path)?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn from_raw(raw: RawAppManifest, path: &Path) -> Result<Self> {
        let program = raw.worker.program.ok_or_else(|| {
            CoreError::ManifestInvalid(format!("{}: worker.program is required", path.display()))
        })?;
        let args = raw.worker.args.ok_or_else(|| {
            CoreError::ManifestInvalid(format!("{}: worker.args is required", path.display()))
        })?;

        let permissions = match raw.permissions {
            Some(raw_perms) => {
                let frontend_dispatch = match raw_perms.frontend_dispatch {
                    Some(raw_fd) => {
                        let methods = raw_fd.allowed_methods.unwrap_or_default();
                        FrontendDispatchPermissions {
                            allowed_methods: methods,
                        }
                    }
                    None => FrontendDispatchPermissions::default(),
                };
                AppPermissions { frontend_dispatch }
            }
            None => AppPermissions::default(),
        };

        Ok(Self {
            app_id: raw.app_id,
            worker: WorkerCommand {
                program,
                args,
                env: raw.worker.env,
                cwd: raw.worker.cwd,
            },
            permissions,
            idle_timeout_ms: raw.idle_timeout_ms,
            startup_timeout_ms: raw.startup_timeout_ms,
        })
    }

    fn validate(&self, path: &Path) -> Result<()> {
        if self.app_id.as_str().trim().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: app_id is required",
                path.display()
            )));
        }

        if self.worker.program.trim().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: worker.program is required",
                path.display()
            )));
        }

        for method in &self.permissions.frontend_dispatch.allowed_methods {
            if method.trim().is_empty() {
                return Err(CoreError::ManifestInvalid(format!(
                    "{}: permissions.frontend_dispatch.allowed_methods contains blank method",
                    path.display()
                )));
            }
        }

        Ok(())
    }
}

impl AppRegistry {
    pub fn load(paths: &KunkkaPaths) -> Result<Self> {
        let apps_dir = paths.config_dir.join("apps");
        if !apps_dir.exists() {
            return Ok(Self::default());
        }

        let mut manifest_paths = Vec::new();
        for entry in fs::read_dir(&apps_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(CoreError::ManifestInvalid(format!(
                    "{}: not a file",
                    path.display()
                )));
            }

            manifest_paths.push(path);
        }
        manifest_paths.sort();

        let mut manifests = BTreeMap::new();
        for path in manifest_paths {
            let manifest = AppManifest::load_file(&path)?;
            match manifests.entry(manifest.app_id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(manifest);
                }
                Entry::Occupied(_) => {
                    return Err(CoreError::ManifestInvalid(format!(
                        "duplicate app_id {} in {}",
                        manifest.app_id.as_str(),
                        path.display()
                    )));
                }
            }
        }

        Ok(Self { manifests })
    }

    pub fn get(&self, app_id: &str) -> Option<&AppManifest> {
        self.manifests.get(&AppId::new(app_id))
    }

    pub fn get_app(&self, app_id: &AppId) -> Option<&AppManifest> {
        self.manifests.get(app_id)
    }

    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

fn default_idle_timeout_ms() -> u64 {
    DEFAULT_IDLE_TIMEOUT_MS
}

fn default_startup_timeout_ms() -> u64 {
    DEFAULT_STARTUP_TIMEOUT_MS
}
