// FILE: src/main.rs

use std::net::SocketAddr;
use tonic::transport::Server;
use tokio_stream::wrappers::TcpListenerStream;

mod llm_client;
mod project_builder;
mod prompt_builder;
mod server;
mod spec;

// Import the servicer from the now-declared server module.
use server::RustPluginServicer;

// Define the generated gRPC module.
pub mod spex_plugin {
    tonic::include_proto!("plugin");
}
use spex_plugin::spex_plugin_server::SpexPluginServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the tracing subscriber to write all logs to stderr.
    // This keeps stdout clean for the IPC handshake.
    tracing_subscriber::fmt()
       .with_writer(std::io::stderr)
       .init();

    // Bind to an available TCP port on the loopback address.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;

    // Construct the handshake string that the Python host expects.
    let handshake_string = format!("1|1|tcp|{}:{}|grpc", addr.ip(), addr.port());

    // Create an instance of our gRPC service implementation.
    let plugin_service = RustPluginServicer::new()?;
    let server = SpexPluginServer::new(plugin_service);

    // Print the handshake string to stdout. This is the only thing
    // that should be printed to stdout during the entire process lifecycle.
    println!("{}", handshake_string);

    // Start the gRPC server.
    Server::builder()
       .add_service(server)
       .serve_with_incoming(TcpListenerStream::new(listener))
       .await?;

    Ok(())
}