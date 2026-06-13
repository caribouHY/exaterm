mod backend;
pub(crate) mod client;
pub(crate) mod control;
mod service;
mod stdio;

pub use backend::McpRuntime;
pub use control::{McpCredentialState, McpLogControlState};
pub use stdio::{run_stdio_proxy, spawn_gui_control_plane};

#[cfg(test)]
mod tests;
