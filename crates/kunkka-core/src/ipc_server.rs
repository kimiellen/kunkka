use crate::xdg::KunkkaPaths;
use crate::Result;
use kunkka_ipc::{IpcConnection, IpcListener};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CoreIpcServer {
    listener: IpcListener,
    socket_path: PathBuf,
}

impl CoreIpcServer {
    pub async fn bind(paths: &KunkkaPaths) -> Result<Self> {
        remove_existing_socket_file(&paths.socket_path)?;

        let listener = IpcListener::bind(&paths.socket_path).await?;

        Ok(Self {
            listener,
            socket_path: paths.socket_path.clone(),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn accept_one(&self) -> Result<IpcConnection> {
        Ok(self.listener.accept().await?)
    }

    pub async fn run(self) -> Result<()> {
        loop {
            let _connection = self.accept_one().await?;
        }
    }
}

fn remove_existing_socket_file(socket_path: &Path) -> Result<()> {
    if socket_path.exists() {
        fs::remove_file(socket_path)?;
    }

    Ok(())
}
