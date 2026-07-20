pub(crate) mod commands;
mod model;
mod state;

pub use commands::{emit_workspace_updated, emit_workspace_updates, emit_workspace_window_closed};
pub use model::{WorkspaceConnectionInfo, WorkspaceTabRegisterInput};
pub use state::WorkspaceState;

#[cfg(test)]
mod tests;
