use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(not(windows))]
use std::fs::{self, File, OpenOptions};

use serde_json::Value;
use tokio::time;

use crate::mcp::control::{
    control_call_over_stream, control_probe_over_stream, CONTROL_UNAVAILABLE_MESSAGE,
};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const POST_LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default)]
pub(crate) struct ControlClient;

impl ControlClient {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn discover_or_start_gui(&self) -> Result<(), String> {
        if self.wait_for_control_plane(DISCOVERY_TIMEOUT).await.is_ok() {
            return Ok(());
        }

        let launch_lock = LaunchLock::acquire()?;
        if launch_lock.is_some() {
            start_gui_process()?;
        }

        self.wait_for_control_plane(POST_LAUNCH_TIMEOUT)
            .await
            .map_err(|_| CONTROL_UNAVAILABLE_MESSAGE.to_string())
    }

    pub(crate) async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, rmcp::ErrorData> {
        call_local_control_tool(tool_name, args).await
    }

    async fn wait_for_control_plane(&self, timeout: Duration) -> Result<(), String> {
        let deadline = time::Instant::now() + timeout;
        loop {
            if probe_local_control_plane().await.is_ok() {
                return Ok(());
            }
            if time::Instant::now() >= deadline {
                return Err(CONTROL_UNAVAILABLE_MESSAGE.into());
            }
            time::sleep(RETRY_INTERVAL).await;
        }
    }
}

fn start_gui_process() -> Result<(), String> {
    let gui_path = resolve_gui_exe_path()?;
    Command::new(&gui_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!(
                "ExaTerm GUI launch failed at {}: {error}",
                gui_path.display()
            )
        })?;
    Ok(())
}

fn resolve_gui_exe_path() -> Result<PathBuf, String> {
    let current_exe =
        std::env::current_exe().map_err(|error| format!("Cannot resolve client path: {error}"))?;
    let current_dir = current_exe
        .parent()
        .ok_or_else(|| "Cannot resolve client directory".to_string())?;

    for dir in current_dir.ancestors().take(4) {
        for name in gui_executable_names() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err("Cannot find ExaTerm GUI executable near the external control client".into())
}

#[cfg(windows)]
fn gui_executable_names() -> &'static [&'static str] {
    &["exaterm.exe", "ExaTerm.exe"]
}

#[cfg(not(windows))]
fn gui_executable_names() -> &'static [&'static str] {
    &["exaterm", "ExaTerm"]
}

#[cfg(windows)]
struct LaunchLock {
    handle: windows_sys::Win32::Foundation::HANDLE,
    acquired: bool,
}

#[cfg(not(windows))]
struct LaunchLock {
    path: PathBuf,
    _file: File,
}

#[cfg(windows)]
impl LaunchLock {
    fn acquire() -> Result<Option<Self>, String> {
        use windows_sys::Win32::{
            Foundation::{
                CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, WAIT_ABANDONED, WAIT_OBJECT_0,
                WAIT_TIMEOUT,
            },
            System::Threading::{CreateMutexW, WaitForSingleObject},
        };

        let name = launch_lock_name();
        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, name.as_ptr()) };
        if handle.is_null() {
            return Err("Cannot create ExaTerm launch mutex".into());
        }

        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            return Ok(Some(Self {
                handle,
                acquired: true,
            }));
        }

        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(Self {
                handle,
                acquired: true,
            })),
            WAIT_TIMEOUT => {
                unsafe { CloseHandle(handle) };
                Ok(None)
            }
            other => {
                unsafe { CloseHandle(handle) };
                Err(format!(
                    "Cannot acquire ExaTerm launch mutex: wait result {other}"
                ))
            }
        }
    }
}

#[cfg(not(windows))]
impl LaunchLock {
    fn acquire() -> Result<Option<Self>, String> {
        let path = launch_lock_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Cannot create ExaTerm data directory: {error}"))?;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => Ok(Some(Self { path, _file: file })),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
            Err(error) => Err(format!("Cannot create ExaTerm launch lock: {error}")),
        }
    }
}

#[cfg(windows)]
impl Drop for LaunchLock {
    fn drop(&mut self) {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::ReleaseMutex};

        unsafe {
            if self.acquired {
                ReleaseMutex(self.handle);
            }
            CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
impl Drop for LaunchLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
fn launch_lock_name() -> Vec<u16> {
    format!(r"Local\ExaTermControlLaunch-{}", current_user_scope_hash())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(windows))]
fn launch_lock_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("exaterm-control-launch.lock")
}

#[cfg(windows)]
async fn call_local_control_tool(tool_name: &str, args: Value) -> Result<Value, rmcp::ErrorData> {
    let stream = connect_named_pipe().await?;
    control_call_over_stream(stream, tool_name, args).await
}

#[cfg(not(windows))]
async fn call_local_control_tool(tool_name: &str, args: Value) -> Result<Value, rmcp::ErrorData> {
    let stream = tokio::net::TcpStream::connect(control_tcp_address())
        .await
        .map_err(|_| rmcp::ErrorData::internal_error(CONTROL_UNAVAILABLE_MESSAGE, None))?;
    control_call_over_stream(stream, tool_name, args).await
}

#[cfg(windows)]
async fn probe_local_control_plane() -> Result<(), String> {
    let stream = connect_named_pipe()
        .await
        .map_err(|error| error.message.into_owned())?;
    control_probe_over_stream(stream).await
}

#[cfg(not(windows))]
async fn probe_local_control_plane() -> Result<(), String> {
    let stream = tokio::net::TcpStream::connect(control_tcp_address())
        .await
        .map_err(|error| format!("ExaTerm control TCP connect error: {error}"))?;
    control_probe_over_stream(stream).await
}

#[cfg(windows)]
async fn connect_named_pipe(
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, rmcp::ErrorData> {
    use tokio::net::windows::named_pipe::ClientOptions;

    ClientOptions::new()
        .open(control_pipe_name())
        .map_err(|_| rmcp::ErrorData::internal_error(CONTROL_UNAVAILABLE_MESSAGE, None))
}

#[cfg(windows)]
pub(super) fn control_pipe_name() -> String {
    format!(
        r"\\.\pipe\exaterm-mcp-control-{}",
        current_user_scope_hash()
    )
}

#[cfg(not(windows))]
pub(super) fn control_tcp_address() -> String {
    let port = 39_500 + (current_user_scope_hash_value() % 1_000) as u16;
    format!("127.0.0.1:{port}")
}

fn current_user_scope_hash() -> String {
    format!("{:016x}", current_user_scope_hash_value())
}

fn current_user_scope_hash_value() -> u64 {
    let user = std::env::var("USERDOMAIN")
        .ok()
        .zip(std::env::var("USERNAME").ok())
        .map(|(domain, user)| format!("{domain}\\{user}"))
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "unknown-user".into())
        .to_ascii_lowercase();
    fnv1a_64(user.as_bytes())
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
