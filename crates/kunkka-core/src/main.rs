use kunkka_core::xdg::KunkkaPaths;

#[tokio::main]
async fn main() -> kunkka_core::Result<()> {
    let paths = KunkkaPaths::resolve()?;
    let socket_path = paths.socket_path.clone();
    let server = kunkka_core::prepare_core_server(&paths).await?;

    println!("kunkka-core listening on {}", socket_path.display());

    server.run().await
}
