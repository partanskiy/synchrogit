pub mod client;
pub mod protocol;
#[cfg_attr(windows, path = "server_windows.rs")]
pub mod server;
pub mod socket;

pub use socket::{default_socket_path, discover_socket_path};

#[cfg(windows)]
mod windows_pipe;

mod connection;
