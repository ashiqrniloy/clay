use std::{env, error::Error, path::PathBuf};

use clay::{
    ipc::default_socket_path,
    server::{IpcServer, ServerConfig},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let socket_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_socket_path);

    eprintln!("clay server listening on {}", socket_path.display());
    IpcServer::new(ServerConfig::new(socket_path)).run().await?;
    Ok(())
}
