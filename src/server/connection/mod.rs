//! Connection handlers for different transports

pub mod websocket;

#[cfg(unix)]
pub mod unix_socket;

#[cfg(windows)]
pub mod named_pipe;
