pub mod app_manifest;
pub mod capability;
pub mod database;
pub mod error;
pub mod ipc_server;
pub mod permissions;
pub mod runtime;
pub mod worker_dispatch;
pub mod worker_registry;
pub mod xdg;

pub use error::{CoreError, Result};
pub use kunkka_ipc as ipc;

use ipc_server::CoreIpcServer;
use runtime::CoreRuntime;
use xdg::KunkkaPaths;

pub async fn prepare_core_server(paths: &KunkkaPaths) -> Result<CoreIpcServer> {
    paths.ensure_dirs()?;
    CoreIpcServer::bind(paths).await
}

pub async fn prepare_core_runtime(paths: &KunkkaPaths) -> Result<CoreRuntime> {
    CoreRuntime::prepare(paths).await
}
