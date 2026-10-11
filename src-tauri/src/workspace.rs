pub(crate) mod commands;
mod model;
mod state;

pub use commands::{emit_workspace_updated, emit_workspace_updates, emit_workspace_window_closed};
#[cfg(test)]
pub(crate) use model::WorkspaceTabMetadataPatch;
pub use model::{WorkspaceConnectionInfo, WorkspaceSnapshot, WorkspaceTabRegisterInput};
pub use state::WorkspaceState;

#[cfg(test)]
mod tests;
