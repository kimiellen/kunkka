use kunkka_core::xdg::KunkkaPaths;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> kunkka_core::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let paths = KunkkaPaths::resolve()?;
    let socket_path = paths.socket_path.clone();
    let runtime = kunkka_core::prepare_core_runtime(&paths).await?;

    tracing::info!(socket = %socket_path.display(), "kunkka-core listening");

    runtime.run().await
}
