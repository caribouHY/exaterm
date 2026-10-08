use super::*;
use std::sync::{Arc, Mutex, OnceLock};

use russh::keys::{PrivateKey, PublicKey};
use russh::{server, ChannelId, Pty};
use tokio::sync::mpsc;

use crate::connect_attempt::ConnectAttemptState;

#[derive(Clone, Copy)]
enum Reply {
    Success,
    Failure,
    Silent,
    Eof,
    Close,
    Disconnect,
    Delayed,
}

#[derive(Clone)]
struct TestServer {
    pty: Reply,
    shell: Reply,
    requests: mpsc::UnboundedSender<(&'static str, server::Handle, ChannelId)>,
    target: Option<Box<TestServer>>,
}

fn host_key() -> PrivateKey {
    static KEY: OnceLock<PrivateKey> = OnceLock::new();
    KEY.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-ssh-shell");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(uuid::Uuid::new_v4().to_string());
        let mut command = std::process::Command::new("ssh-keygen");
        command
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        assert!(command.status().unwrap().success());
        let key = russh::keys::load_secret_key(&path, None).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("pub")).unwrap();
        key
    })
    .clone()
}

fn server_config() -> Arc<server::Config> {
    Arc::new(server::Config {
        keys: vec![host_key()],
        auth_rejection_time: Duration::ZERO,
        ..Default::default()
    })
}

impl TestServer {
    fn reply(
        &self,
        reply: Reply,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        match reply {
            Reply::Success => {
                session.data(channel, b"banner".to_vec())?;
                session.extended_data(channel, 1, b"notice".to_vec())?;
                session.channel_success(channel)
            }
            Reply::Failure => session.channel_failure(channel),
            Reply::Eof => session.eof(channel),
            Reply::Close => session.close(channel),
            Reply::Disconnect => session.disconnect(Disconnect::ByApplication, "fixture", "en"),
            Reply::Silent => {
                let handle = session.handle();
                tokio::spawn(async move {
                    let mut tick = tokio::time::interval(Duration::from_millis(20));
                    loop {
                        tick.tick().await;
                        if handle.data(channel, b"pending".to_vec()).await.is_err() {
                            break;
                        }
                    }
                });
                Ok(())
            }
            Reply::Delayed => Ok(()),
        }
    }
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<server::Auth, Self::Error> {
        match user {
            "pty-reject" => self.pty = Reply::Failure,
            "shell-reject" => self.shell = Reply::Failure,
            "pty-ignore" => self.pty = Reply::Silent,
            "shell-ignore" => self.shell = Reply::Silent,
            "cancel-pty" => self.pty = Reply::Delayed,
            "cancel-shell" => self.shell = Reply::Delayed,
            _ => {}
        }
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<server::Msg>,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        cols: u32,
        rows: u32,
        _width: u32,
        _height: u32,
        _modes: &[(Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!((cols, rows), (80, 24));
        self.requests
            .send(("pty", session.handle(), channel))
            .unwrap();
        self.reply(self.pty, channel, session)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.requests
            .send(("shell", session.handle(), channel))
            .unwrap();
        self.reply(self.shell, channel, session)
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        _host: &str,
        _port: u32,
        _origin: &str,
        _origin_port: u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        let target = self.target.take().unwrap();
        tokio::spawn(async move {
            server::run_stream(server_config(), channel.into_stream(), *target)
                .await
                .unwrap()
                .await
                .ok();
        });
        Ok(true)
    }
}

#[derive(Clone, Default)]
struct TestClient {
    output: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl client::Handler for TestClient {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(key.key_data() == host_key().public_key().key_data())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.output.lock().unwrap().push(data.to_vec());
        Ok(())
    }

    async fn extended_data(
        &mut self,
        channel: ChannelId,
        _ext: u32,
        data: &[u8],
        session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        self.data(channel, data, session).await
    }
}

struct Fixture {
    handle: client::Handle<TestClient>,
    jump: Option<client::Handle<TestClient>>,
    requests: mpsc::UnboundedReceiver<(&'static str, server::Handle, ChannelId)>,
    output: Arc<Mutex<Vec<Vec<u8>>>>,
}

async fn fixture(pty: Reply, shell: Reply, jump: bool) -> Fixture {
    let (requests, rx) = mpsc::unbounded_channel();
    let target = TestServer {
        pty,
        shell,
        requests: requests.clone(),
        target: None,
    };
    let server = if jump {
        TestServer {
            pty,
            shell,
            requests,
            target: Some(Box::new(target)),
        }
    } else {
        target
    };
    let (client_stream, server_stream) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        server::run_stream(server_config(), server_stream, server)
            .await
            .unwrap()
            .await
            .ok();
    });
    let client = TestClient::default();
    let output = client.output.clone();
    let config = Arc::new(client::Config::default());
    let transport_client = if jump {
        TestClient::default()
    } else {
        client.clone()
    };
    let mut handle = client::connect_stream(config.clone(), client_stream, transport_client)
        .await
        .unwrap();
    assert!(handle.authenticate_none("fixture").await.unwrap().success());
    let jump = if jump {
        let tunnel = handle
            .channel_open_direct_tcpip("fixture", 22, "fixture", 0)
            .await
            .unwrap();
        let mut target = client::connect_stream(config, tunnel.into_stream(), client)
            .await
            .unwrap();
        assert!(target.authenticate_none("fixture").await.unwrap().success());
        Some(std::mem::replace(&mut handle, target))
    } else {
        None
    };
    Fixture {
        handle,
        jump,
        requests: rx,
        output,
    }
}

async fn assert_released(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.handle.is_closed() || fixture.jump.as_ref().is_some_and(|j| !j.is_closed()) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("target and jump transports must terminate");
}

#[tokio::test]
async fn success_confirms_both_requests_without_duplicating_output() {
    for jump in [false, true] {
        let mut fixture = fixture(Reply::Success, Reply::Success, jump).await;
        let channel = start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None)
            .await
            .unwrap();
        assert_eq!(fixture.requests.recv().await.unwrap().0, "pty");
        assert_eq!(fixture.requests.recv().await.unwrap().0, "shell");
        assert_eq!(
            *fixture.output.lock().unwrap(),
            vec![
                b"banner".to_vec(),
                b"notice".to_vec(),
                b"banner".to_vec(),
                b"notice".to_vec()
            ]
        );
        let terminals = crate::terminal_control::TerminalControlState::new();
        terminals
            .register_session(
                "fixture".into(),
                crate::terminal_control::TerminalProtocol::Ssh,
                "fixture".into(),
            )
            .await;
        let (tx, rx) = mpsc::channel(8);
        for data in fixture.output.lock().unwrap().clone() {
            tx.try_send(crate::ssh::io::SshReadRequest {
                data,
                stream_kind: crate::ssh::io::SshReadStreamKind::Data,
            })
            .unwrap();
        }
        drop(tx);
        crate::ssh::io::process_ssh_output(&terminals, "fixture", rx, |_, _| {}).await;
        assert_eq!(
            terminals.read_output("fixture", 100).await.unwrap().output,
            "bannernoticebannernotice"
        );
        cleanup_failed_connection(Some(&channel), &fixture.handle, fixture.jump.as_ref()).await;
        drop(channel);
        assert_released(&fixture).await;
    }
}

#[tokio::test]
#[ignore = "local endpoint for manual optimized-executable validation"]
async fn optimized_runtime_endpoint() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sh03-runtime");
    std::fs::create_dir_all(&root).unwrap();
    let key = host_key().public_key().to_openssh().unwrap();
    std::fs::write(
        root.join("endpoint.json"),
        serde_json::to_vec(&serde_json::json!({
            "port": listener.local_addr().unwrap().port(), "public_key": key,
        }))
        .unwrap(),
    )
    .unwrap();
    let (requests, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let deadline = tokio::time::sleep(Duration::from_secs(600));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted.unwrap();
                let server = TestServer { pty: Reply::Success, shell: Reply::Success,
                    requests: requests.clone(), target: None };
                tokio::spawn(async move {
                    if let Ok(session) = server::run_stream(server_config(), stream, server).await {
                        let _ = session.await;
                    }
                });
            }
        }
    }
}

#[tokio::test]
async fn rejection_and_early_termination_fail_at_the_current_stage() {
    for jump in [false, true] {
        for reply in [Reply::Failure, Reply::Eof, Reply::Close, Reply::Disconnect] {
            for pty_stage in [true, false] {
                let mut fixture = fixture(
                    if pty_stage { reply } else { Reply::Success },
                    if pty_stage { Reply::Success } else { reply },
                    jump,
                )
                .await;
                let error =
                    start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None)
                        .await
                        .err()
                        .unwrap();
                assert_eq!(
                    error,
                    if pty_stage {
                        "PTY request failed"
                    } else {
                        "Shell request failed"
                    }
                );
                assert_released(&fixture).await;
                assert_eq!(fixture.requests.recv().await.unwrap().0, "pty");
                if !pty_stage {
                    assert_eq!(fixture.requests.recv().await.unwrap().0, "shell");
                }
                assert!(fixture.requests.try_recv().is_err());
            }
        }
    }
}

async fn assert_timeout(pty_stage: bool) {
    for jump in [false, true] {
        let fixture = fixture(
            if pty_stage {
                Reply::Silent
            } else {
                Reply::Success
            },
            if pty_stage {
                Reply::Success
            } else {
                Reply::Silent
            },
            jump,
        )
        .await;
        let began = tokio::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(13),
            start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None),
        )
        .await
        .expect("output must not extend the request deadline");
        assert_eq!(
            result.err().unwrap(),
            if pty_stage {
                SSH_PTY_TIMEOUT_ERROR
            } else {
                SSH_SHELL_TIMEOUT_ERROR
            }
        );
        assert!(began.elapsed() >= Duration::from_secs(10));
        assert_released(&fixture).await;
    }
}

#[tokio::test]
async fn pty_without_reply_times_out_despite_output() {
    assert_timeout(true).await;
}

#[tokio::test]
async fn shell_without_reply_times_out_despite_output() {
    assert_timeout(false).await;
}

#[tokio::test]
async fn cancellation_releases_resources_and_ignores_late_success() {
    for jump in [false, true] {
        for pty_stage in [true, false] {
            let mut fixture = fixture(
                if pty_stage {
                    Reply::Delayed
                } else {
                    Reply::Success
                },
                if pty_stage {
                    Reply::Success
                } else {
                    Reply::Delayed
                },
                jump,
            )
            .await;
            let state =
                ConnectAttemptState::new("The SSH connection attempt was cancelled", "duplicate");
            let mut attempt = state.register("fixture".into()).unwrap();
            let operation = start_session(
                &fixture.handle,
                fixture.jump.as_ref(),
                80,
                24,
                None,
                Some(&mut attempt),
            );
            let cancel = async {
                let mut request = fixture.requests.recv().await.unwrap();
                if !pty_stage {
                    request = fixture.requests.recv().await.unwrap();
                }
                assert!(state.cancel("fixture"));
                let _ = request.1.channel_success(request.2).await;
            };
            let (result, ()) = tokio::join!(operation, cancel);
            assert_eq!(
                result.err().unwrap(),
                "The SSH connection attempt was cancelled"
            );
            assert_released(&fixture).await;
            assert!(!attempt.begin_completion());
        }
    }
}

#[tokio::test]
async fn failure_does_not_prevent_a_fresh_successful_attempt() {
    let failed = fixture(Reply::Failure, Reply::Success, false).await;
    assert!(
        start_session(&failed.handle, failed.jump.as_ref(), 80, 24, None, None)
            .await
            .is_err()
    );
    assert_released(&failed).await;
    let retry = fixture(Reply::Success, Reply::Success, false).await;
    let state = ConnectAttemptState::new("The SSH connection attempt was cancelled", "duplicate");
    let mut attempt = state.register("retry".into()).unwrap();
    let channel = start_session(
        &retry.handle,
        retry.jump.as_ref(),
        80,
        24,
        None,
        Some(&mut attempt),
    )
    .await
    .unwrap();
    assert!(!state.cancel("retry"));
    cleanup_failed_connection(Some(&channel), &retry.handle, retry.jump.as_ref()).await;
    assert_released(&retry).await;
}
