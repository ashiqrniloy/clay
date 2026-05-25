use std::{env, error::Error};

use clay::ipc::{IpcEndpoint, default_endpoint};
#[cfg(unix)]
use clay::server::{IpcServer, ServerConfig};

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = env::args_os()
        .nth(1)
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    eprintln!("clay server listening on {endpoint}");
    IpcServer::new(ServerConfig::new(endpoint.as_unix_socket_path()))
        .run()
        .await?;
    Ok(())
}

#[cfg(not(unix))]
fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = env::args_os()
        .nth(1)
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    Err(format!(
        "Clay server IPC is currently implemented only for Unix sockets; unsupported endpoint {endpoint}"
    )
    .into())
}
