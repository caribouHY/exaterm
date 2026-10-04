use super::*;
use std::borrow::Cow;
use std::io;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use russh::keys::{Algorithm, PublicKey};
use russh::{client, server, Channel};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};

fn generated_key(kind: &str) -> PrivateKey {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-ssh-auth");
    fs::create_dir_all(&root).unwrap();
    let dir = root.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&dir).unwrap();
    let path = dir.join("key");
    let status = Command::new("ssh-keygen")
        .args([
            "-q",
            "-t",
            kind,
            "-b",
            if kind == "ecdsa" { "256" } else { "2048" },
            "-N",
            "",
            "-f",
        ])
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let key = load_private_key_for_auth(path.to_str().unwrap(), None).unwrap();
    fs::remove_dir_all(dir).unwrap();
    key
}

fn rsa_key() -> PrivateKey {
    static KEY: OnceLock<PrivateKey> = OnceLock::new();
    KEY.get_or_init(|| generated_key("rsa")).clone()
}

fn host_key() -> PrivateKey {
    static KEY: OnceLock<PrivateKey> = OnceLock::new();
    KEY.get_or_init(|| generated_key("ed25519")).clone()
}

#[derive(Default)]
struct Requests {
    pending: Vec<u8>,
    banner_read: bool,
    offered: Vec<String>,
    signed: Vec<String>,
}

fn read_string<'a>(packet: &mut &'a [u8]) -> &'a [u8] {
    let len = u32::from_be_bytes(packet[..4].try_into().unwrap()) as usize;
    let value = &packet[4..4 + len];
    *packet = &packet[4 + len..];
    value
}

impl Requests {
    fn record(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        if !self.banner_read {
            let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') else {
                return;
            };
            self.pending.drain(..=end);
            self.banner_read = true;
        }
        while self.pending.len() >= 5 {
            let len = u32::from_be_bytes(self.pending[..4].try_into().unwrap()) as usize;
            if self.pending.len() < 4 + len {
                return;
            }
            let padding = usize::from(self.pending[4]);
            let mut payload = &self.pending[5..4 + len - padding];
            if payload.first() == Some(&50) {
                payload = &payload[1..];
                read_string(&mut payload);
                read_string(&mut payload);
                if read_string(&mut payload) == b"publickey" {
                    let signed = payload[0] != 0;
                    payload = &payload[1..];
                    let algorithm = String::from_utf8(read_string(&mut payload).to_vec()).unwrap();
                    if signed {
                        read_string(&mut payload);
                        let mut signature = read_string(&mut payload);
                        assert_eq!(read_string(&mut signature), algorithm.as_bytes());
                        self.signed.push(algorithm);
                    } else {
                        self.offered.push(algorithm);
                    }
                }
            }
            self.pending.drain(..4 + len);
        }
    }
}

// Only synthetic in-memory test connections use unencrypted packets for observation.
struct ObservedStream {
    inner: DuplexStream,
    requests: Arc<Mutex<Requests>>,
}

impl AsyncRead for ObservedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for ObservedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(written)) = &result {
            self.requests.lock().unwrap().record(&buf[..*written]);
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct TestClient;

impl client::Handler for TestClient {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(key.key_data() == host_key().public_key().key_data())
    }
}

#[derive(Clone)]
struct TestServer {
    accepted: &'static str,
    requests: Arc<Mutex<Requests>>,
    reject: bool,
    target: Option<Box<TestServer>>,
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_publickey_offered(
        &mut self,
        _user: &str,
        _key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        if self
            .requests
            .lock()
            .unwrap()
            .offered
            .last()
            .map(String::as_str)
            == Some(self.accepted)
            && !self.reject
        {
            Ok(server::Auth::Accept)
        } else {
            Ok(server::Auth::reject())
        }
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        _host: &str,
        _port: u32,
        _originator: &str,
        _originator_port: u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        let Some(target) = self.target.take() else {
            return Ok(false);
        };
        tokio::spawn(async move {
            server::run_stream(
                server_config(target.accepted),
                channel.into_stream(),
                *target,
            )
            .await
            .unwrap()
            .await
            .ok();
        });
        Ok(true)
    }
}

fn server_config(accepted: &str) -> Arc<server::Config> {
    let mut config = server::Config {
        keys: vec![host_key()],
        methods: MethodSet::from(&[MethodKind::PublicKey][..]),
        auth_rejection_time: Duration::ZERO,
        ..server::Config::default()
    };
    config.preferred.key = Cow::Owned(vec![Algorithm::Ed25519, accepted.parse().unwrap()]);
    config.preferred.cipher = Cow::Borrowed(&[russh::cipher::NONE]);
    config.preferred.mac = Cow::Borrowed(&[russh::mac::NONE]);
    Arc::new(config)
}

fn client_config(extension: bool) -> Arc<client::Config> {
    let mut config = client::Config::default();
    config.preferred.cipher = Cow::Borrowed(&[russh::cipher::NONE]);
    config.preferred.mac = Cow::Borrowed(&[russh::mac::NONE]);
    if !extension {
        config.preferred.kex = Cow::Owned(
            config
                .preferred
                .kex
                .iter()
                .copied()
                .filter(|name| *name != russh::kex::EXTENSION_SUPPORT_AS_CLIENT)
                .collect(),
        );
    }
    Arc::new(config)
}

async fn connect(server: TestServer, extension: bool) -> client::Handle<TestClient> {
    let config = server_config(server.accepted);
    connect_with_config(server, extension, config).await
}

async fn connect_with_config(
    server: TestServer,
    extension: bool,
    config: Arc<server::Config>,
) -> client::Handle<TestClient> {
    let (client_stream, server_stream) = tokio::io::duplex(65536);
    let observed = ObservedStream {
        inner: client_stream,
        requests: Arc::clone(&server.requests),
    };
    tokio::spawn(async move {
        server::run_stream(config, server_stream, server)
            .await
            .unwrap()
            .await
            .ok();
    });
    client::connect_stream(client_config(extension), observed, TestClient)
        .await
        .unwrap()
}

async fn assert_auth(accepted: &'static str, extension: bool, key: PrivateKey, expected: &[&str]) {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let mut handle = connect(
        TestServer {
            accepted,
            requests: Arc::clone(&requests),
            reject: false,
            target: None,
        },
        extension,
    )
    .await;
    let result = authenticate_public_key(&mut handle, "synthetic", key)
        .await
        .unwrap();
    assert!(result.success());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.offered, expected);
    assert_eq!(requests.signed, [accepted]);
}

#[tokio::test]
async fn rsa_sha2_and_legacy_server_selection() {
    for algorithm in ["rsa-sha2-512", "rsa-sha2-256", "ssh-rsa"] {
        assert_auth(algorithm, true, rsa_key(), &[algorithm]).await;
    }
}

#[tokio::test]
async fn rsa_prefers_sha512_when_both_sha2_algorithms_are_advertised() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let server = TestServer {
        accepted: "rsa-sha2-512",
        requests: Arc::clone(&requests),
        reject: false,
        target: None,
    };
    let mut config = server_config(server.accepted);
    Arc::get_mut(&mut config)
        .unwrap()
        .preferred
        .key
        .to_mut()
        .push(Algorithm::Rsa {
            hash: Some(HashAlg::Sha256),
        });
    let mut handle = connect_with_config(server, true, config).await;
    assert!(authenticate_public_key(&mut handle, "synthetic", rsa_key())
        .await
        .unwrap()
        .success());
    assert_eq!(requests.lock().unwrap().signed, ["rsa-sha2-512"]);
}

#[tokio::test]
async fn support_discovery_is_inside_the_authentication_deadline() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let mut handle = connect(
        TestServer {
            accepted: "ssh-rsa",
            requests: Arc::clone(&requests),
            reject: false,
            target: None,
        },
        false,
    )
    .await;
    let error = authenticate_public_key_with_timeout(
        &mut handle,
        "synthetic",
        rsa_key(),
        Duration::from_millis(30),
    )
    .await
    .unwrap_err();
    assert_eq!(error, SSH_AUTH_TIMEOUT_ERROR);
    assert!(requests.lock().unwrap().offered.is_empty());
}

#[tokio::test]
async fn missing_extension_tries_sha2_before_legacy() {
    assert_auth("rsa-sha2-512", false, rsa_key(), &["rsa-sha2-512"]).await;
    assert_auth(
        "rsa-sha2-256",
        false,
        rsa_key(),
        &["rsa-sha2-512", "rsa-sha2-256"],
    )
    .await;
    assert_auth(
        "ssh-rsa",
        false,
        rsa_key(),
        &["rsa-sha2-512", "rsa-sha2-256", "ssh-rsa"],
    )
    .await;
}

#[tokio::test]
async fn fallback_attempts_share_the_support_discovery_deadline() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let server = TestServer {
        accepted: "ssh-rsa",
        requests: Arc::clone(&requests),
        reject: false,
        target: None,
    };
    let mut config = server_config(server.accepted);
    Arc::get_mut(&mut config).unwrap().auth_rejection_time = Duration::from_millis(450);
    let mut handle = connect_with_config(server, false, config).await;
    let error = authenticate_public_key_with_timeout(
        &mut handle,
        "synthetic",
        rsa_key(),
        Duration::from_millis(1700),
    )
    .await
    .unwrap_err();
    assert_eq!(error, SSH_AUTH_TIMEOUT_ERROR);
    let requests = requests.lock().unwrap();
    assert!(!requests.offered.is_empty());
    assert!(!requests
        .offered
        .iter()
        .any(|algorithm| algorithm == "ssh-rsa"));
    assert!(requests.signed.is_empty());
}

#[tokio::test]
async fn advertised_sha2_rejection_does_not_downgrade() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let mut handle = connect(
        TestServer {
            accepted: "rsa-sha2-512",
            requests: Arc::clone(&requests),
            reject: true,
            target: None,
        },
        true,
    )
    .await;
    assert!(
        !authenticate_public_key(&mut handle, "synthetic", rsa_key())
            .await
            .unwrap()
            .success()
    );
    assert_eq!(requests.lock().unwrap().offered, ["rsa-sha2-512"]);
}

#[tokio::test]
async fn non_rsa_keys_keep_their_signature_algorithm() {
    assert_auth("ssh-ed25519", true, host_key(), &["ssh-ed25519"]).await;
    assert_auth(
        "ecdsa-sha2-nistp256",
        true,
        generated_key("ecdsa"),
        &["ecdsa-sha2-nistp256"],
    )
    .await;
}

#[test]
fn retry_requires_non_partial_public_key_failure() {
    assert!(!should_retry_public_key_hash(&AuthResult::Success));
    for (methods, partial, retry) in [
        (vec![MethodKind::PublicKey], false, true),
        (vec![MethodKind::PublicKey], true, false),
        (vec![MethodKind::KeyboardInteractive], false, false),
        (vec![], false, false),
    ] {
        assert_eq!(
            should_retry_public_key_hash(&AuthResult::Failure {
                remaining_methods: MethodSet::from(methods.as_slice()),
                partial_success: partial,
            }),
            retry
        );
    }
}

#[tokio::test]
async fn jump_and_target_select_independent_rsa_hashes() {
    let jump_requests = Arc::new(Mutex::new(Requests::default()));
    let target_requests = Arc::new(Mutex::new(Requests::default()));
    let target = TestServer {
        accepted: "rsa-sha2-256",
        requests: Arc::clone(&target_requests),
        reject: false,
        target: None,
    };
    let mut jump = connect(
        TestServer {
            accepted: "rsa-sha2-512",
            requests: Arc::clone(&jump_requests),
            reject: false,
            target: Some(Box::new(target)),
        },
        true,
    )
    .await;
    assert!(authenticate_public_key(&mut jump, "synthetic", rsa_key())
        .await
        .unwrap()
        .success());
    let channel = jump
        .channel_open_direct_tcpip("synthetic", 22, "synthetic", 0)
        .await
        .unwrap();
    let (observed, forwarded) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        let mut channel = channel.into_stream();
        let mut forwarded = forwarded;
        let _ = tokio::io::copy_bidirectional(&mut channel, &mut forwarded).await;
    });
    let mut target = client::connect_stream(
        client_config(true),
        ObservedStream {
            inner: observed,
            requests: Arc::clone(&target_requests),
        },
        TestClient,
    )
    .await
    .unwrap();
    assert!(authenticate_public_key(&mut target, "synthetic", rsa_key())
        .await
        .unwrap()
        .success());
    assert_eq!(jump_requests.lock().unwrap().signed, ["rsa-sha2-512"]);
    assert_eq!(target_requests.lock().unwrap().signed, ["rsa-sha2-256"]);
}
