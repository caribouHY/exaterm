use super::*;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::watch;

pub(super) trait SessionEvents: Send + Sync + 'static {
    fn connected(&self, session_id: &str);
    fn data(&self, session_id: &str, output: crate::terminal_control::TerminalOutputEvent);
    fn error(&self, session_id: &str, error: &str);
    fn disconnected(&self, session_id: &str, snapshot: Option<crate::workspace::WorkspaceSnapshot>);
}

pub(super) struct SessionOwner {
    pub terminals: TerminalControlState,
    pub workspace: WorkspaceState,
    pub logger: Option<LoggerState>,
    pub events: Arc<dyn SessionEvents>,
    pub session_id: String,
}

pub(super) struct TelnetSession {
    pub writer: mpsc::Sender<Vec<u8>>,
    codec: Arc<Mutex<TelnetParser>>,
    stop: watch::Sender<bool>,
    done: watch::Receiver<bool>,
}

pub(super) async fn connect<S, F>(
    state: &TelnetState,
    owner: SessionOwner,
    request: TelnetConnectRequest,
    mut attempt: Option<ConnectAttempt>,
    operation: std::pin::Pin<Box<F>>,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    F: std::future::Future<Output = Result<S, String>>,
{
    let stream = run_with_attempt(attempt.as_ref(), operation).await?;
    register(state, owner, request, stream, attempt.as_mut()).await
}

pub(super) async fn register<S>(
    state: &TelnetState,
    owner: SessionOwner,
    request: TelnetConnectRequest,
    stream: S,
    attempt: Option<&mut ConnectAttempt>,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut sessions = state.sessions.lock().await;
    if attempt.is_some_and(|attempt| !attempt.begin_completion()) {
        return Err(TELNET_CONNECT_CANCELLED.to_string());
    }
    owner
        .terminals
        .register_session_with_encoding(
            owner.session_id.clone(),
            crate::terminal_control::TerminalProtocol::Telnet,
            format!("{}:{}", request.host, request.port),
            request.encoding,
        )
        .await;
    let (writer, write_rx) = mpsc::channel(64);
    let codec = Arc::new(Mutex::new(TelnetParser::new(request.cols, request.rows)));
    let (stop, stop_rx) = watch::channel(false);
    let (done_tx, done) = watch::channel(false);
    let session = TelnetSession {
        writer: writer.clone(),
        codec: codec.clone(),
        stop,
        done,
    };
    // No suspension after output registration: publication and I/O startup form
    // one ownership transfer, even if the caller drops its connect future.
    sessions.insert(owner.session_id.clone(), session);
    owner.events.connected(&owner.session_id);
    let sessions = state.sessions.clone();
    tokio::spawn(async move {
        let result = run_io(stream, writer, write_rx, codec, stop_rx, &owner).await;
        if let Err(error) = result {
            owner
                .terminals
                .capture_notice(&owner.session_id, &error)
                .await;
            owner.events.error(&owner.session_id, &error);
        }
        owner.terminals.mark_disconnected(&owner.session_id).await;
        let snapshot = owner.workspace.mark_disconnected(&owner.session_id).await;
        if let Some(logger) = &owner.logger {
            crate::logger::clear_session_logs(logger, &owner.session_id).await;
        }
        owner.events.disconnected(&owner.session_id, snapshot);
        // Keep the completion receiver discoverable until all finalization
        // effects finish so concurrent disconnects wait for the same owner.
        sessions.lock().await.remove(&owner.session_id);
        done_tx.send_replace(true);
    });
    Ok(())
}

async fn run_io<S>(
    stream: S,
    writer: mpsc::Sender<Vec<u8>>,
    mut write_rx: mpsc::Receiver<Vec<u8>>,
    codec: Arc<Mutex<TelnetParser>>,
    mut stop: watch::Receiver<bool>,
    owner: &SessionOwner,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut reader, mut output) = tokio::io::split(stream);
    let read = async {
        let mut buf = [0; 4096];
        loop {
            let n = reader.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                return Ok::<_, String>(());
            }
            // Keep response generation and enqueue ordered with resize, even
            // when retained-output/log capture suspends in between.
            let mut parser = codec.lock().await;
            let (data, response) = parser.parse(&buf[..n]);
            if !data.is_empty() {
                if let Some(event) = owner
                    .terminals
                    .append_output(&owner.session_id, &data)
                    .await
                {
                    owner.events.data(&owner.session_id, event);
                }
            }
            if !response.is_empty() {
                writer.send(response).await.map_err(|e| e.to_string())?;
            }
            drop(parser);
        }
    };
    let write = async {
        while let Some(request) = write_rx.recv().await {
            output
                .write_all(&request)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    tokio::select! {
        biased;
        _ = stop.wait_for(|stopped| *stopped) => Ok(()),
        result = write => result,
        result = read => result,
    }
}

pub(super) async fn disconnect(state: &TelnetState, session_id: &str) -> bool {
    let completion = {
        let sessions = state.sessions.lock().await;
        sessions.get(session_id).map(|session| {
            session.stop.send_replace(true);
            session.done.clone()
        })
    };
    if let Some(mut done) = completion {
        let _ = done.wait_for(|finished| *finished).await;
        true
    } else {
        false
    }
}

pub(super) async fn resize(
    state: &TelnetState,
    session_id: &str,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let (writer, codec) = {
        let sessions = state.sessions.lock().await;
        let session = sessions.get(session_id).ok_or("Session not found")?;
        (session.writer.clone(), session.codec.clone())
    };
    // Match the reader's codec -> queue order: reserving before this lock
    // would let concurrent resizes consume every slot without enqueuing.
    let mut codec = codec.lock().await;
    let permit = writer
        .reserve()
        .await
        .map_err(|e| format!("Failed to send the resize request: {}", e))?;
    permit.send(codec.resize(cols, rows));
    Ok(())
}

#[cfg(test)]
mod tests;
