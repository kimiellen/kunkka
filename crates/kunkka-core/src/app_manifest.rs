use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use kunkka_worker_sdk::AppId;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppManifest {
    pub app_id: AppId,
    pub worker: WorkerCommand,
    #[serde(default = "default_idle_timeout_ms")]
    pub idle_timeout_ms: u64,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkerCommand {
    #[serde(default)]
    pub program: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct AppRegistry {
    manifests: BTreeMap<AppId, AppManifest>,
}

impl AppManifest {
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let manifest: Self = serde_json::from_slice(&bytes)
            .map_err(|err| CoreError::ManifestInvalid(format!("{}: {err}", path.display())))?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<()> {
        if self.app_id.as_str().is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: app_id is required",
                path.display()
            )));
        }

        if self.worker.program.is_empty() {
            return Err(CoreError::ManifestInvalid(format!(
                "{}: worker.program is required",
                path.display()
            )));
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

        let mut manifests = BTreeMap::new();
        for entry in fs::read_dir(&apps_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let manifest = AppManifest::load_file(&path)?;
            manifests.insert(manifest.app_id.clone(), manifest);
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
