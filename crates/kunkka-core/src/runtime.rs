use crate::ipc_server::CoreIpcServer;
use crate::worker_registry::{handle_worker_registration_frame, WorkerRegistry};
use crate::xdg::KunkkaPaths;
use crate::Result;

pub struct CoreRuntime {
    server: CoreIpcServer,
    registry: WorkerRegistry,
}

impl CoreRuntime {
    pub async fn prepare(paths: &KunkkaPaths) -> Result<Self> {
        paths.ensure_dirs()?;
        let server = CoreIpcServer::bind(paths).await?;

        Ok(Self {
            server,
            registry: WorkerRegistry::new(),
        })
    }

    pub fn registry(&self) -> &WorkerRegistry {
        &self.registry
    }

    pub async fn run_once(&mut self) -> Result<()> {
        let mut connection = self.server.accept_one().await?;
        let Some(frame) = connection.recv_frame().await? else {
            return Ok(());
        };

        let response = handle_worker_registration_frame(&mut self.registry, frame)?;
        connection.send_frame(&response).await?;

        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            self.run_once().await?;
        }
    }
}
