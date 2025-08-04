use std::net::SocketAddr;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

// Only the server module is needed now.
mod server;

use server::MySpexPlugin as RustPlugin;

// Import the auto-generated gRPC types.
pub mod spex_plugin {
    tonic::include_proto!("plugin");
}
use spex_plugin::spex_plugin_server::SpexPluginServer;

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("panic: {info}");
        if let Some(loc) = info.location() {
            eprintln!("at: {}:{}", loc.file(), loc.line());
        }
    }));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Keep stdout clean for the handshake required by spex-core.
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    install_panic_hook();

    // Bind to an ephemeral port on the loopback address.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;

    // Print the handshake line to stdout.
    println!("1|1|tcp|{}:{}|grpc", addr.ip(), addr.port());

    // The server struct from `src/server.rs` no longer needs arguments.
    let plugin_service = RustPlugin::default();
    let server = SpexPluginServer::new(plugin_service);

    Server::builder()
        .add_service(server)
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    Ok(())
}