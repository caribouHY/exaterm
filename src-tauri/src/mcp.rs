mod backend;
pub(crate) mod control;
mod http_transport;
mod service;

pub use backend::McpRuntime;
pub use control::{McpCredentialState, McpLogControlState};
pub use http_transport::spawn_mcp_server;

#[cfg(test)]
mod tests;
