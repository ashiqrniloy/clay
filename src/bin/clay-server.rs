use std::{env, error::Error};

use clay::ipc::{IpcEndpoint, default_endpoint};
#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};

#[cfg(any(unix, windows))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = env::args_os()
        .nth(1)
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    eprintln!("clay server listening on {endpoint}");
    IpcServer::new(ServerConfig::new(endpoint)).run().await?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = env::args_os()
        .nth(1)
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}
