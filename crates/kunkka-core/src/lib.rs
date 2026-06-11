pub mod error;
pub mod ipc_server;
pub mod worker_registry;
pub mod xdg;

pub use error::{CoreError, Result};
pub use kunkka_ipc as ipc;

use ipc_server::CoreIpcServer;
use xdg::KunkkaPaths;

pub async fn prepare_core_server(paths: &KunkkaPaths) -> Result<CoreIpcServer> {
    paths.ensure_dirs()?;
    CoreIpcServer::bind(paths).await
}
