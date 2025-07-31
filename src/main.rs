use std::net::SocketAddr;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

mod llm_client;
mod project_builder;
mod prompt_builder;
mod server;
mod spec;

use server::RustPluginServicer;

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
    // stderr logging; keep stdout clean for handshake
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    install_panic_hook();

    // Bind to an ephemeral port on loopback
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;

    // Handshake to stdout (required by spex-core)
    println!("1|1|tcp|{}:{}|grpc", addr.ip(), addr.port());

    // Build and run gRPC server
    let plugin_service = RustPluginServicer::new()?;
    let server = SpexPluginServer::new(plugin_service);

    Server::builder()
        .add_service(server)
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await?;

    Ok(())
}