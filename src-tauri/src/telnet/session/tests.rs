use super::*;
use crate::terminal_control::{TerminalOutputEvent, TerminalStatus};
use std::sync::Mutex as StdMutex;
use std::time::Duration;

#[derive(Default)]
struct Events(StdMutex<Vec<String>>);

impl SessionEvents for Events {
    fn connected(&self, _: &str) {
        self.0.lock().unwrap().push("connected".into());
    }
    fn data(&self, _: &str, event: TerminalOutputEvent) {
        self.0.lock().unwrap().push(format!(
            "data:{}:{}:{}",
            event.start_cursor, event.cursor, event.output
        ));
    }
    fn error(&self, _: &str, _: &str) {
        self.0.lock().unwrap().push("error".into());
    }
    fn disconnected(&self, _: &str, _: Option<crate::workspace::WorkspaceSnapshot>) {
        self.0.lock().unwrap().push("disconnected".into());
    }
}

struct Fixture {
    state: TelnetState,
    terminals: TerminalControlState,
    workspace: WorkspaceState,
    events: Arc<Events>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            state: TelnetState::new(),
            terminals: TerminalControlState::new(),
            workspace: WorkspaceState::new(),
            events: Arc::new(Events::default()),
        }
    }
    fn owner(&self) -> SessionOwner {
        SessionOwner {
            terminals: self.terminals.clone(),
            workspace: self.workspace.clone(),
            logger: None,
            events: self.events.clone(),
            session_id: "s1".into(),
        }
    }
    fn request(&self) -> TelnetConnectRequest {
        TelnetConnectRequest {
            host: "synthetic".into(),
            port: 23,
            cols: 80,
            rows: 24,
            encoding: None,
        }
    }
    async fn start<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        &self,
        stream: S,
    ) -> watch::Receiver<bool> {
        register(&self.state, self.owner(), self.request(), stream, None)
            .await
            .unwrap();
        self.state
            .sessions
            .lock()
            .await
            .get("s1")
            .unwrap()
            .done
            .clone()
    }
    async fn finished(&self, mut done: watch::Receiver<bool>) {
        tokio::time::timeout(Duration::from_secs(3), done.wait_for(|done| *done))
            .await
            .unwrap()
            .unwrap();
        assert!(self.state.sessions.lock().await.is_empty());
        assert_eq!(
            self.terminals.session_info("s1").await.unwrap().status,
            TerminalStatus::Disconnected
        );
        assert_eq!(
            self.events
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|s| *s == "disconnected")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn immediate_banner_negotiation_then_eof_preserve_output_and_order() {
    let fixture = Fixture::new();
    let (stream, mut peer) = tokio::io::duplex(4096);
    peer.write_all("A😀界".as_bytes()).await.unwrap();
    peer.write_all(&[255, 253, 31]).await.unwrap();
    let done = fixture.start(stream).await;
    let mut response = Vec::new();
    response.resize(12, 0);
    peer.read_exact(&mut response).await.unwrap();
    peer.shutdown().await.unwrap();
    let mut expected = vec![255, 251, 31];
    expected.extend(build_naws(80, 24));
    assert_eq!(response, expected);
    fixture.finished(done).await;
    let snapshot = fixture.terminals.read_output("s1", 100).await.unwrap();
    assert_eq!(snapshot.output, "A😀界");
    assert_eq!(snapshot.cursor, 3);
    assert_eq!(
        *fixture.events.0.lock().unwrap(),
        ["connected", "data:0:3:A😀界", "disconnected"]
    );
}

#[tokio::test]
async fn registration_lock_blocks_all_io_and_cancellation_releases_stream() {
    let fixture = Arc::new(Fixture::new());
    let (stream, mut peer) = tokio::io::duplex(4096);
    peer.write_all(b"banner").await.unwrap();
    let attempt = fixture
        .state
        .connect_attempts
        .register("setup".into())
        .unwrap();
    let guard = fixture.state.sessions.lock().await;
    let task_fixture = fixture.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut attempt = attempt;
        started_tx.send(()).unwrap();
        register(
            &task_fixture.state,
            task_fixture.owner(),
            task_fixture.request(),
            stream,
            Some(&mut attempt),
        )
        .await
    });
    started_rx.await.unwrap();
    assert!(fixture.terminals.list_sessions().await.is_empty());
    assert!(fixture.events.0.lock().unwrap().is_empty());
    assert!(fixture.state.connect_attempts.cancel("setup"));
    drop(guard);
    assert_eq!(task.await.unwrap().unwrap_err(), TELNET_CONNECT_CANCELLED);
    let mut buf = [0; 1];
    assert_eq!(peer.read(&mut buf).await.unwrap(), 0);
    assert!(fixture.terminals.list_sessions().await.is_empty());
    assert!(fixture.state.sessions.lock().await.is_empty());
}

#[tokio::test]
async fn successful_registration_rejects_late_cancellation_and_uses_latest_resize() {
    let fixture = Fixture::new();
    let (stream, mut peer) = tokio::io::duplex(4096);
    let mut attempt = fixture
        .state
        .connect_attempts
        .register("setup".into())
        .unwrap();
    register(
        &fixture.state,
        fixture.owner(),
        fixture.request(),
        stream,
        Some(&mut attempt),
    )
    .await
    .unwrap();
    assert!(!fixture.state.connect_attempts.cancel("setup"));
    let done = fixture
        .state
        .sessions
        .lock()
        .await
        .get("s1")
        .unwrap()
        .done
        .clone();
    resize(&fixture.state, "s1", 255, 511).await.unwrap();
    let expected = build_naws(255, 511);
    let mut buf = vec![0; expected.len()];
    peer.read_exact(&mut buf).await.unwrap();
    assert_eq!(buf, expected);
    peer.write_all(&[255, 253, 31]).await.unwrap();
    let mut expected = vec![255, 251, 31];
    expected.extend(build_naws(255, 511));
    let mut buf = vec![0; expected.len()];
    peer.read_exact(&mut buf).await.unwrap();
    assert_eq!(buf, expected);
    disconnect(&fixture.state, "s1").await;
    fixture.finished(done).await;
}

#[tokio::test]
async fn concurrent_disconnects_and_eof_share_finalization() {
    let fixture = Fixture::new();
    let (stream, peer) = tokio::io::duplex(4096);
    let done = fixture.start(stream).await;
    drop(peer);
    tokio::join!(
        disconnect(&fixture.state, "s1"),
        disconnect(&fixture.state, "s1")
    );
    fixture.finished(done).await;
    assert_eq!(
        *fixture.events.0.lock().unwrap(),
        ["connected", "disconnected"]
    );
}

#[tokio::test]
async fn blocked_user_write_does_not_block_banner_or_eof() {
    let fixture = Fixture::new();
    let (stream, mut peer) = tokio::io::duplex(16);
    let done = fixture.start(stream).await;
    let writer = fixture
        .state
        .sessions
        .lock()
        .await
        .get("s1")
        .unwrap()
        .writer
        .clone();
    writer.send(vec![0; 1024]).await.unwrap();
    // Receiving one byte proves write_all started and cannot finish until the
    // peer reads the remaining bytes. Its read side deliberately stays idle.
    let mut first = [0; 1];
    peer.read_exact(&mut first).await.unwrap();
    peer.write_all(&[255, 253, 31]).await.unwrap();
    peer.write_all(b"banner").await.unwrap();
    peer.shutdown().await.unwrap();
    fixture.finished(done).await;
    assert_eq!(
        fixture
            .terminals
            .read_output("s1", 100)
            .await
            .unwrap()
            .output,
        "banner"
    );
}

struct CaptureBarrier {
    events: Arc<Events>,
    entered: StdMutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: StdMutex<std::sync::mpsc::Receiver<()>>,
}

impl SessionEvents for CaptureBarrier {
    fn connected(&self, id: &str) {
        self.events.connected(id);
    }
    fn data(&self, id: &str, output: TerminalOutputEvent) {
        self.events.data(id, output);
        if let Some(entered) = self.entered.lock().unwrap().take() {
            entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
        }
    }
    fn error(&self, id: &str, error: &str) {
        self.events.error(id, error);
    }
    fn disconnected(&self, id: &str, snapshot: Option<crate::workspace::WorkspaceSnapshot>) {
        self.events.disconnected(id, snapshot);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resize_cannot_overtake_a_negotiation_suspended_in_output_capture() {
    let fixture = Fixture::new();
    let (stream, mut peer) = tokio::io::duplex(4096);
    let (entered_tx, entered) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let mut owner = fixture.owner();
    owner.events = Arc::new(CaptureBarrier {
        events: fixture.events.clone(),
        entered: StdMutex::new(Some(entered_tx)),
        release: StdMutex::new(release_rx),
    });
    register(&fixture.state, owner, fixture.request(), stream, None)
        .await
        .unwrap();
    let done = fixture
        .state
        .sessions
        .lock()
        .await
        .get("s1")
        .unwrap()
        .done
        .clone();
    peer.write_all(&[b'A', 255, 253, 31]).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), entered)
        .await
        .unwrap()
        .unwrap();
    let mut resizes: Vec<_> = (0..64)
        .map(|_| Box::pin(resize(&fixture.state, "s1", 132, 43)))
        .collect();
    let mut blocked = true;
    for resize in &mut resizes {
        blocked &= futures::poll!(resize).is_pending();
    }
    let capacity = fixture
        .state
        .sessions
        .lock()
        .await
        .get("s1")
        .unwrap()
        .writer
        .capacity();
    release.send(()).unwrap();
    let results = tokio::time::timeout(Duration::from_secs(3), futures::future::join_all(resizes))
        .await
        .unwrap();
    for result in results {
        result.unwrap();
    }
    assert!(
        blocked,
        "resize must wait for the pending negotiation enqueue"
    );
    assert_eq!(
        capacity, 64,
        "resizes must not reserve slots before locking the codec"
    );
    let mut expected = vec![255, 251, 31];
    expected.extend(build_naws(80, 24));
    for _ in 0..64 {
        expected.extend(build_naws(132, 43));
    }
    let mut received = vec![0; expected.len()];
    tokio::time::timeout(Duration::from_secs(3), peer.read_exact(&mut received))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, expected);
    peer.shutdown().await.unwrap();
    fixture.finished(done).await;
}

struct FaultStream {
    read: tokio::sync::oneshot::Receiver<()>,
    write: tokio::sync::oneshot::Receiver<()>,
    dropped: Arc<std::sync::atomic::AtomicUsize>,
}

impl AsyncRead for FaultStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::future::Future;
        std::pin::Pin::new(&mut self.read)
            .poll(cx)
            .map(|_| Err(std::io::Error::other("synthetic read failure")))
    }
}
impl AsyncWrite for FaultStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        use std::future::Future;
        std::pin::Pin::new(&mut self.write)
            .poll(cx)
            .map(|_| Err(std::io::Error::other("synthetic write failure")))
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}
impl Drop for FaultStream {
    fn drop(&mut self) {
        self.dropped
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[tokio::test]
async fn reader_writer_failures_and_disconnect_release_io_once() {
    for failure in ["reader", "writer", "both", "cancel"] {
        let fixture = Fixture::new();
        let (read_tx, read) = tokio::sync::oneshot::channel();
        let (write_tx, write) = tokio::sync::oneshot::channel();
        let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let done = fixture
            .start(FaultStream {
                read,
                write,
                dropped: dropped.clone(),
            })
            .await;
        let writer = fixture
            .state
            .sessions
            .lock()
            .await
            .get("s1")
            .unwrap()
            .writer
            .clone();
        writer.send(vec![1]).await.unwrap();
        if failure == "reader" || failure == "both" {
            read_tx.send(()).unwrap();
        }
        if failure == "writer" || failure == "both" {
            write_tx.send(()).unwrap();
        }
        if failure == "cancel" {
            tokio::join!(
                disconnect(&fixture.state, "s1"),
                disconnect(&fixture.state, "s1")
            );
        }
        fixture.finished(done).await;
        assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(writer.send(vec![2]).await.is_err());
        assert_eq!(
            fixture
                .events
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| *e == "error")
                .count(),
            usize::from(failure != "cancel")
        );
    }
}

#[tokio::test]
async fn setup_failure_and_cancellation_do_not_publish_sessions() {
    let fixture = Fixture::new();
    let attempt = fixture
        .state
        .connect_attempts
        .register("failed".into())
        .unwrap();
    let result = connect(
        &fixture.state,
        fixture.owner(),
        fixture.request(),
        Some(attempt),
        Box::pin(async {
            Err::<tokio::io::DuplexStream, _>("synthetic setup failure".to_string())
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), "synthetic setup failure");
    assert!(!fixture.state.connect_attempts.cancel("failed"));

    let attempt = fixture
        .state
        .connect_attempts
        .register("cancelled".into())
        .unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (_read_tx, read) = tokio::sync::oneshot::channel();
    let (_write_tx, write) = tokio::sync::oneshot::channel();
    let stream = FaultStream {
        read,
        write,
        dropped: dropped.clone(),
    };
    let operation = async move {
        let _stream = stream;
        started_tx.send(()).unwrap();
        std::future::pending::<Result<FaultStream, String>>().await
    };
    let cancel = async {
        started_rx.await.unwrap();
        assert!(fixture.state.connect_attempts.cancel("cancelled"));
    };
    let (result, ()) = tokio::join!(
        connect(
            &fixture.state,
            fixture.owner(),
            fixture.request(),
            Some(attempt),
            Box::pin(operation)
        ),
        cancel,
    );
    assert_eq!(result.unwrap_err(), TELNET_CONNECT_CANCELLED);
    assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!fixture.state.connect_attempts.cancel("cancelled"));
    assert!(fixture.state.sessions.lock().await.is_empty());
    assert!(fixture.terminals.list_sessions().await.is_empty());
    assert!(fixture.events.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn immediate_eof_before_workspace_registration_stays_disconnected() {
    let fixture = Fixture::new();
    let (stream, mut peer) = tokio::io::duplex(4096);
    peer.write_all("即時😀".as_bytes()).await.unwrap();
    peer.shutdown().await.unwrap();
    let done = fixture.start(stream).await;
    fixture.finished(done).await;
    let snapshot = fixture
        .workspace
        .register_session_tab(
            crate::workspace::WorkspaceTabRegisterInput {
                window_id: Some("main".into()),
                tab_id: None,
                session_id: "s1".into(),
                connection_type: crate::terminal_control::TerminalProtocol::Telnet,
                title: "synthetic".into(),
                encoding: "utf-8".into(),
                terminal_mode: "general".into(),
                connection_info: None,
                is_manual_logging: false,
                manual_log_file_path: None,
            },
            &fixture.terminals,
        )
        .await;
    assert!(!snapshot.tabs[0].is_connected);
    let output = fixture.terminals.read_output("s1", 100).await.unwrap();
    assert_eq!(output.output, "即時😀");
    assert_eq!(output.cursor, 3);
}

#[tokio::test]
async fn loopback_tcp_exercises_production_connect_owner_and_resize() {
    let fixture = Fixture::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (negotiated, negotiation) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut peer, _) = listener.accept().await.unwrap();
        peer.write_all("Loopback😀".as_bytes()).await.unwrap();
        peer.write_all(&[255, 253, 31]).await.unwrap();
        let mut expected = vec![255, 251, 31];
        expected.extend(build_naws(80, 24));
        let mut received = vec![0; expected.len()];
        peer.read_exact(&mut received).await.unwrap();
        assert_eq!(received, expected);
        negotiated.send(()).unwrap();
        let expected = build_naws(132, 43);
        let mut received = vec![0; expected.len()];
        peer.read_exact(&mut received).await.unwrap();
        assert_eq!(received, expected);
        peer.write_all(&[255, 253, 31]).await.unwrap();
        let mut expected = vec![255, 251, 31];
        expected.extend(build_naws(132, 43));
        let mut received = vec![0; expected.len()];
        peer.read_exact(&mut received).await.unwrap();
        assert_eq!(received, expected);
        peer.shutdown().await.unwrap();
    });
    connect(
        &fixture.state,
        fixture.owner(),
        fixture.request(),
        None,
        Box::pin(async move { TcpStream::connect(address).await.map_err(|e| e.to_string()) }),
    )
    .await
    .unwrap();
    let done = fixture
        .state
        .sessions
        .lock()
        .await
        .get("s1")
        .unwrap()
        .done
        .clone();
    tokio::time::timeout(Duration::from_secs(3), negotiation)
        .await
        .unwrap()
        .unwrap();
    resize(&fixture.state, "s1", 132, 43).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    fixture.finished(done).await;
    assert_eq!(
        fixture
            .terminals
            .read_output("s1", 100)
            .await
            .unwrap()
            .output,
        "Loopback😀"
    );
}
