pub(crate) mod client;
pub(crate) mod protocol;
mod server;
pub(crate) mod service;

pub use protocol::{ExternalControlCredentialState, ExternalControlLogControlState};
pub use server::spawn_gui_control_plane;
pub use service::{ExternalControlRuntime, ExternalControlService};

pub(crate) use service::{
    ConnectSavedProfileArgs, ConnectSerialConsoleArgs, ExternalControlError,
    ExternalControlRequest, ExternalControlResponse, ReadTerminalOutputArgs,
    RunTerminalCommandArgs, SendTerminalInputArgs, StartTerminalLogArgs, StopTerminalLogArgs,
};
