mod backend;
pub(crate) mod control;
mod http_transport;
mod service;
mod stdio;

pub use backend::McpRuntime;
pub use control::{McpCredentialState, McpLogControlState};
pub use http_transport::spawn_mcp_server;
pub use stdio::{run_stdio_proxy, spawn_gui_control_plane};

#[cfg(test)]
mod tests;
