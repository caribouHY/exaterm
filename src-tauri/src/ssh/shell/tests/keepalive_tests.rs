use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct PollObserved {
    stream: tokio::io::DuplexStream,
    polls: Arc<std::sync::atomic::AtomicUsize>,
}

impl tokio::io::AsyncRead for PollObserved {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.polls.fetch_add(1, Ordering::Relaxed);
        std::pin::Pin::new(&mut self.stream).poll_read(cx, buffer)
    }
}

impl tokio::io::AsyncWrite for PollObserved {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, bytes)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keepalive_authentication_wait_does_not_spin() {
    let (requests, _rx) = mpsc::unbounded_channel();
    let server = TestServer {
        pty: Reply::Success,
        shell: Reply::Success,
        requests,
        target: None,
        runtime: false,
    };
    let (stream, server_stream) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        let session = server::run_stream(server_config(), server_stream, server)
            .await
            .unwrap();
        let _ = session.await;
    });
    let polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let config = crate::ssh::client_config::build_client_config(&crate::config::SshConfig {
        keepalive_enabled: true,
        keepalive_interval_secs: 1,
        ..Default::default()
    })
    .unwrap();
    let mut handle = client::connect_stream(
        Arc::new(config),
        PollObserved {
            stream,
            polls: polls.clone(),
        },
        TestClient::default(),
    )
    .await
    .unwrap();
    let before = polls.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(1400)).await;
    let idle_polls = polls.load(Ordering::Relaxed) - before;
    assert!(handle.authenticate_none("fixture").await.unwrap().success());
    handle
        .disconnect(Disconnect::ByApplication, "fixture complete", "en")
        .await
        .unwrap();
    assert!(
        idle_polls < 1000,
        "authentication wait must stay idle, observed {idle_polls} read polls"
    );
}

struct ObservedTransport {
    discard_replies: Arc<AtomicBool>,
    packets: mpsc::UnboundedReceiver<()>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for ObservedTransport {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn observed_transport() -> (
    tokio::io::DuplexStream,
    tokio::io::DuplexStream,
    ObservedTransport,
) {
    let (client, proxy_client) = tokio::io::duplex(65536);
    let (proxy_server, server) = tokio::io::duplex(65536);
    let (mut client_read, mut client_write) = tokio::io::split(proxy_client);
    let (mut server_read, mut server_write) = tokio::io::split(proxy_server);
    let discard_replies = Arc::new(AtomicBool::new(false));
    let discard = discard_replies.clone();
    let (tx, packets) = mpsc::unbounded_channel();
    let upstream = tokio::spawn(async move {
        let mut buffer = [0; 65536];
        while let Ok(count) = client_read.read(&mut buffer).await {
            if count == 0 || server_write.write_all(&buffer[..count]).await.is_err() {
                break;
            }
            let _ = tx.send(());
        }
    });
    let downstream = tokio::spawn(async move {
        let mut buffer = [0; 65536];
        while let Ok(count) = server_read.read(&mut buffer).await {
            if count == 0 {
                break;
            }
            if !discard.load(Ordering::SeqCst)
                && client_write.write_all(&buffer[..count]).await.is_err()
            {
                break;
            }
        }
    });
    (
        client,
        server,
        ObservedTransport {
            discard_replies,
            packets,
            tasks: vec![upstream, downstream],
        },
    )
}

async fn keepalive_fixture(jump: bool, enabled: bool) -> (Fixture, ObservedTransport) {
    let (requests, rx) = mpsc::unbounded_channel();
    let target = TestServer {
        pty: Reply::Success,
        shell: Reply::Success,
        requests: requests.clone(),
        target: None,
        runtime: false,
    };
    let server = if jump {
        TestServer {
            target: Some(Box::new(target)),
            runtime: false,
            pty: Reply::Success,
            shell: Reply::Success,
            requests,
        }
    } else {
        target
    };
    let (client_stream, server_stream, transport) = observed_transport();
    tokio::spawn(async move {
        if let Ok(session) = server::run_stream(server_config(), server_stream, server).await {
            let _ = session.await;
        }
    });
    let config = Arc::new(
        crate::ssh::client_config::build_client_config(&crate::config::SshConfig {
            keepalive_enabled: enabled,
            keepalive_interval_secs: 1,
            keepalive_max: 2,
            ..Default::default()
        })
        .unwrap(),
    );
    let client = TestClient::default();
    let output = client.output.clone();
    let mut handle = client::connect_stream(
        config.clone(),
        client_stream,
        if jump {
            TestClient::default()
        } else {
            client.clone()
        },
    )
    .await
    .unwrap();
    assert!(handle.authenticate_none("fixture").await.unwrap().success());
    let jump_handle = if jump {
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
    (
        Fixture {
            handle,
            jump: jump_handle,
            requests: rx,
            output,
        },
        transport,
    )
}

#[tokio::test]
async fn keepalive_preserves_idle_direct_and_jump_sessions_without_terminal_data() {
    for jump in [false, true] {
        let (fixture, mut transport) = keepalive_fixture(jump, true).await;
        let channel = start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None)
            .await
            .unwrap();
        while transport.packets.try_recv().is_ok() {}
        let mut writes = 0;
        // Observe longer than the three-second silent-peer deadline, regardless of packet batching.
        assert!(tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                transport.packets.recv().await.unwrap();
                writes += 1;
            }
        })
        .await
        .is_err());
        assert!(writes >= 2);
        assert!(!fixture.handle.is_closed());
        assert_eq!(
            *fixture.output.lock().unwrap(),
            vec![
                b"banner".to_vec(),
                b"notice".to_vec(),
                b"banner".to_vec(),
                b"notice".to_vec()
            ]
        );
        assert!(fixture
            .jump
            .as_ref()
            .is_none_or(|handle| !handle.is_closed()));
        cleanup_failed_connection(Some(&channel), &fixture.handle, fixture.jump.as_ref()).await;
        assert_released(&fixture).await;
    }
}

#[tokio::test]
async fn disabled_keepalive_does_not_probe_idle_connections() {
    for jump in [false, true] {
        let (fixture, mut transport) = keepalive_fixture(jump, false).await;
        let channel = start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None)
            .await
            .unwrap();
        while transport.packets.try_recv().is_ok() {}
        assert!(
            tokio::time::timeout(Duration::from_secs(4), transport.packets.recv())
                .await
                .is_err()
        );
        assert!(!fixture.handle.is_closed());
        cleanup_failed_connection(Some(&channel), &fixture.handle, fixture.jump.as_ref()).await;
        assert_released(&fixture).await;
    }
}

#[tokio::test]
async fn keepalive_detects_lost_replies_on_direct_and_jump_connections() {
    for jump in [false, true] {
        let (fixture, transport) = keepalive_fixture(jump, true).await;
        let channel = start_session(&fixture.handle, fixture.jump.as_ref(), 80, 24, None, None)
            .await
            .unwrap();
        transport.discard_replies.store(true, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(8), async {
            while !fixture.handle.is_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("missing replies must close the target transport");
        cleanup_failed_connection(Some(&channel), &fixture.handle, fixture.jump.as_ref()).await;
        assert_released(&fixture).await;
    }
}
