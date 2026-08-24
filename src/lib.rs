pub(crate) mod behavior;
pub mod client;
pub mod client_commands;
pub mod color;
pub mod docs;
pub mod editor;
pub mod ipc;
pub mod packages;
pub mod perf;
pub mod protocol;
pub(crate) mod sanitize;
#[doc(hidden)]
pub mod shell;

#[cfg(any(unix, windows))]
pub mod server;
