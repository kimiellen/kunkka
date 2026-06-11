use kunkka_core::xdg::KunkkaPaths;

#[tokio::main]
async fn main() -> kunkka_core::Result<()> {
    let paths = KunkkaPaths::resolve()?;
    let socket_path = paths.socket_path.clone();
    let runtime = kunkka_core::prepare_core_runtime(&paths).await?;

    println!("kunkka-core listening on {}", socket_path.display());

    runtime.run().await
}
