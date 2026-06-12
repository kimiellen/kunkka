use kunkka_native_host::bridge::NativeHostSession;
use kunkka_native_host::host::run_native_host;
use kunkka_native_host::path::resolve_core_socket_path;

#[tokio::main]
async fn main() -> kunkka_native_host::Result<()> {
    let socket_path = resolve_core_socket_path();
    let mut session = NativeHostSession::new(socket_path);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    run_native_host(&mut reader, &mut writer, &mut session).await
}
