use super::protocol::CONTROL_UNAVAILABLE_MESSAGE;

#[cfg(any(unix, test))]
pub(in crate::external_control) fn require_same_user(
    expected: u32,
    actual: u32,
) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err("External control peer belongs to another OS user".into())
    }
}

#[cfg(unix)]
pub(crate) mod unix {
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt},
        path::{Path, PathBuf},
    };
    use tokio::net::{UnixListener, UnixStream};

    pub(in crate::external_control) fn uid() -> u32 {
        // SAFETY: This call takes no pointers and only reads the effective UID.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe { libc::geteuid() }
    }

    pub(in crate::external_control) fn control_directory() -> PathBuf {
        // A fixed, UID-derived location avoids trusting environment variables for identity.
        PathBuf::from("/tmp").join(format!("exaterm-control-{}", uid()))
    }

    pub(in crate::external_control) fn validate_directory(path: &Path) -> Result<(), String> {
        let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        super::require_same_user(uid(), meta.uid())?;
        if !meta.is_dir() || meta.mode() & 0o777 != 0o700 {
            return Err("External control directory must be private (0700)".into());
        }
        Ok(())
    }

    pub(in crate::external_control) fn prepare_directory(path: &Path) -> Result<(), String> {
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
        validate_directory(path)
    }

    pub(crate) fn verify_peer(stream: &UnixStream) -> Result<(), String> {
        let peer = stream.peer_cred().map_err(|e| e.to_string())?;
        super::require_same_user(uid(), peer.uid())
    }

    pub(in crate::external_control) async fn connect_at(
        directory: &Path,
    ) -> Result<UnixStream, String> {
        validate_directory(directory)?;
        let stream = UnixStream::connect(directory.join("control.sock"))
            .await
            .map_err(|e| e.to_string())?;
        verify_peer(&stream)?;
        Ok(stream)
    }

    pub(in crate::external_control) async fn bind_at(
        directory: &Path,
    ) -> Result<(UnixListener, fs::File), String> {
        prepare_directory(directory)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("listener.lock"))
            .map_err(|e| e.to_string())?;
        let meta = lock.metadata().map_err(|e| e.to_string())?;
        super::require_same_user(uid(), meta.uid())?;
        if !meta.is_file() || meta.mode() & 0o777 != 0o600 {
            return Err("Invalid external control listener lock".into());
        }
        lock.try_lock()
            .map_err(|_| "External control endpoint is already running".to_string())?;
        let path = directory.join("control.sock");
        if let Ok(meta) = fs::symlink_metadata(&path) {
            super::require_same_user(uid(), meta.uid())?;
            if !meta.file_type().is_socket() {
                return Err("External control endpoint is not a socket".into());
            }
            match UnixStream::connect(&path).await {
                Ok(_) => return Err("External control endpoint is already running".into()),
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        UnixListener::bind(path)
            .map(|listener| (listener, lock))
            .map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
pub(crate) mod windows {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
        System::{
            Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: This owner wraps only handles returned by OpenProcess/OpenProcessToken,
                // never borrowed pipe handles or the current-process pseudo-handle.
                // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    fn token_user(process: HANDLE) -> Result<Vec<usize>, String> {
        let mut token = std::ptr::null_mut();
        // SAFETY: The process handle is live for this call; token is writable output and a
        // successful handle transfers to Handle.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err("Cannot inspect external control peer token".into());
        }
        let token = Handle(token);
        let mut length = 0;
        // SAFETY: The token is owned and live; a null buffer with zero length queries only the
        // required size into length.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe {
            GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut length);
        }
        let mut data = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        if length == 0
            // SAFETY: The usize allocation is aligned for TOKEN_USER and covers length bytes;
            // Windows writes within that capacity.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
            || unsafe {
                GetTokenInformation(
                    token.0,
                    TokenUser,
                    data.as_mut_ptr().cast(),
                    length,
                    &mut length,
                )
            } == 0
        {
            return Err("Cannot read external control peer identity".into());
        }
        Ok(data)
    }

    pub(crate) fn verify_peer(pipe: &impl AsRawHandle, server_side: bool) -> Result<(), String> {
        verify_peer_with_identity(pipe, server_side, |pid| {
            // SAFETY: OpenProcess takes scalar inputs; a non-null result is owned by Handle and
            // null is rejected before token access.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
            let process = Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) });
            if process.0.is_null() {
                return Err("Cannot open external control pipe peer".into());
            }
            token_user(process.0)
        })
    }

    fn verify_peer_with_identity<P: AsRawHandle + ?Sized>(
        pipe: &P,
        server_side: bool,
        peer_identity: impl FnOnce(u32) -> Result<Vec<usize>, String>,
    ) -> Result<(), String> {
        let mut pid = 0;
        let handle = pipe.as_raw_handle();
        // SAFETY: The borrowed pipe stays open; pid is initialized writable output for either
        // pipe-process query.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let success = unsafe {
            if server_side {
                GetNamedPipeClientProcessId(handle, &mut pid)
            } else {
                GetNamedPipeServerProcessId(handle, &mut pid)
            }
        };
        if success == 0 {
            return Err("Cannot identify external control pipe peer".into());
        }
        // SAFETY: The OS returns a process pseudo-handle valid for this process; token_user borrows
        // it and never closes it.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let current = token_user(unsafe { GetCurrentProcess() })?;
        let peer = peer_identity(pid)?;
        // SAFETY: Successful TokenUser retrieval initialized this aligned allocation; its owning
        // Vec remains live through EqualSid.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let current = unsafe { &*current.as_ptr().cast::<TOKEN_USER>() };
        // SAFETY: The peer TokenUser allocation is initialized, aligned, and remains owned through
        // the following SID comparison.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let peer = unsafe { &*peer.as_ptr().cast::<TOKEN_USER>() };
        // SAFETY: Both SID pointers refer to OS-initialized data inside the still-live TokenUser
        // allocations.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        if unsafe { EqualSid(current.User.Sid, peer.User.Sid) } == 0 {
            return Err("External control peer belongs to another OS user".into());
        }
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn named_pipe_rejects_foreign_sid_and_identity_lookup_failure() {
            use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
            use windows_sys::Win32::Security::{
                CreateWellKnownSid, WinAnonymousSid, SID_AND_ATTRIBUTES,
            };
            let name = format!(r"\\.\pipe\exaterm-sid-test-{}", uuid::Uuid::new_v4());
            let server = ServerOptions::new().create(&name).unwrap();
            let client = ClientOptions::new().open(&name).unwrap();
            server.connect().await.unwrap();
            for server_side in [true, false] {
                let pipe: &dyn AsRawHandle = if server_side { &server } else { &client };
                let foreign = |_pid| {
                    // Replace token lookup only; pipe PID lookup and SID comparison stay real.
                    let mut data = vec![0usize; 32];
                    let storage = &mut data[4..];
                    let mut size = std::mem::size_of_val(storage) as u32;
                    let sid = storage.as_mut_ptr().cast();
                    assert_ne!(
                        // SAFETY: The aligned SID slice remains owned by data; size is its exact
                        // remaining byte capacity and the domain pointer is unused.
                        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
                        unsafe {
                            CreateWellKnownSid(
                                WinAnonymousSid,
                                std::ptr::null_mut(),
                                sid,
                                &mut size,
                            )
                        },
                        0
                    );
                    // SAFETY: The usize buffer is aligned and large enough for TOKEN_USER; the
                    // following SID region is disjoint and shares its lifetime.
                    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
                    unsafe {
                        data.as_mut_ptr().cast::<TOKEN_USER>().write(TOKEN_USER {
                            User: SID_AND_ATTRIBUTES {
                                Sid: sid,
                                Attributes: 0,
                            },
                        });
                    }
                    Ok(data)
                };
                assert!(verify_peer_with_identity(pipe, server_side, foreign)
                    .unwrap_err()
                    .contains("another OS user"));
                assert!(verify_peer_with_identity(pipe, server_side, |_| Err(
                    "unavailable identity".into()
                ))
                .is_err());
            }
        }
    }
}

pub(in crate::external_control) fn unavailable() -> super::service::ExternalControlError {
    super::service::ExternalControlError::Unavailable(CONTROL_UNAVAILABLE_MESSAGE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_os_user_is_rejected() {
        assert!(require_same_user(1000, 1001).is_err());
        assert!(require_same_user(1000, 1000).is_ok());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn real_named_pipe_checks_both_peer_identities_and_recovers_after_disconnect() {
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        let name = format!(r"\\.\pipe\exaterm-isolated-test-{}", uuid::Uuid::new_v4());
        for _ in 0..2 {
            let server = ServerOptions::new().create(&name).unwrap();
            let client = ClientOptions::new().open(&name).unwrap();
            server.connect().await.unwrap();
            windows::verify_peer(&server, true).unwrap();
            windows::verify_peer(&client, false).unwrap();
            drop(client);
            drop(server);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_unix_endpoint_checks_permissions_peers_stale_socket_and_live_listener() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("private");
        let (listener, lock) = unix::bind_at(&directory).await.unwrap();
        let client = unix::connect_at(&directory).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        unix::verify_peer(&server).unwrap();
        assert!(unix::bind_at(&directory).await.is_err());
        let listener = listener.into_std().unwrap();
        drop((client, server, listener, lock));
        let (listener, _lock) = unix::bind_at(&directory).await.unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(unix::connect_at(&directory).await.is_err());
        drop(listener);
    }

    #[cfg(unix)]
    #[test]
    fn unix_endpoint_rejects_directory_symlinks_and_foreign_owner() {
        let temp = tempfile::tempdir().unwrap();
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(temp.path(), &link).unwrap();
        assert!(unix::validate_directory(&link).is_err());
        // The same comparison is used for directory ownership and kernel peer credentials.
        assert!(require_same_user(unix::uid(), unix::uid().wrapping_add(1)).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn foreign_uid_process_helper() {
        use std::io::{Read, Write};
        let Ok(path) = std::env::var("EXATERM_TEST_SOCKET") else {
            return;
        };
        let mut stream = if std::env::var("EXATERM_TEST_ROLE").unwrap() == "server" {
            let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
            println!("EXATERM_READY");
            std::io::stdout().flush().unwrap();
            listener.accept().unwrap().0
        } else {
            let stream = std::os::unix::net::UnixStream::connect(path).unwrap();
            println!("EXATERM_READY");
            std::io::stdout().flush().unwrap();
            stream
        };
        stream.read_exact(&mut [0]).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[ignore = "requires an isolated Linux root runner to launch a synthetic foreign UID"]
    async fn real_foreign_uid_is_rejected_on_both_sides() {
        use std::{
            io::{BufRead, BufReader},
            os::unix::{fs::PermissionsExt, process::CommandExt},
            process::{Command, Stdio},
        };
        use tokio::io::AsyncWriteExt;
        assert_eq!(
            unix::uid(),
            0,
            "run only in an isolated root test environment"
        );
        for role in ["server", "client"] {
            let temp = tempfile::tempdir().unwrap();
            std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
            let path = temp.path().join("foreign.sock");
            let listener = if role == "client" {
                let listener = tokio::net::UnixListener::bind(&path).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).unwrap();
                Some(listener)
            } else {
                None
            };
            let helper = format!(
                "{}::foreign_uid_process_helper",
                module_path!().split_once("::").unwrap().1
            );
            // Resolve the running executable through the kernel, not a replaceable filesystem path.
            let mut child = Command::new("/proc/self/exe")
                .args(["--exact", &helper, "--nocapture"])
                .env("EXATERM_TEST_SOCKET", &path)
                .env("EXATERM_TEST_ROLE", role)
                .uid(65534)
                .gid(65534)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                assert!(
                    output.read_line(&mut line).unwrap() > 0,
                    "child exited before readiness"
                );
                if line.trim() == "EXATERM_READY" {
                    break;
                }
            }
            let mut stream = if let Some(listener) = listener {
                listener.accept().await.unwrap().0
            } else {
                tokio::net::UnixStream::connect(&path).await.unwrap()
            };
            assert!(unix::verify_peer(&stream)
                .unwrap_err()
                .contains("another OS user"));
            stream.write_all(b"x").await.unwrap();
            assert!(child.wait().unwrap().success());
        }
    }
}
