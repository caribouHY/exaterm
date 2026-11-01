use super::*;

struct TestKey {
    dir: PathBuf,
    path: PathBuf,
}

impl TestKey {
    fn new(passphrase: &str) -> Self {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-ssh-passphrase");
        fs::create_dir_all(&root).unwrap();
        let dir = root.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        let key = Self {
            path: dir.join("id_ed25519"),
            dir,
        };
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-a", "1", "-N", passphrase, "-f"])
            .arg(&key.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("ssh-keygen is required for passphrase tests");
        assert!(status.success(), "ssh-keygen failed with status {status}");
        key
    }

    fn request(&self, method: &str, passphrase: Option<&str>) -> SshAuthRequest {
        build_auth_request(
            Some(method.to_string()),
            String::new(),
            Some(self.path.to_str().unwrap().to_string()),
            passphrase.map(str::to_string),
            None,
        )
        .unwrap()
    }
}

impl Drop for TestKey {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).expect("failed to remove synthetic SSH key");
    }
}

fn request_key(request: SshAuthRequest) -> Result<PrivateKey, String> {
    let (path, passphrase) = match request {
        SshAuthRequest::Auto {
            private_key_path,
            key_passphrase,
        } => (private_key_path.unwrap(), key_passphrase),
        SshAuthRequest::PublicKey {
            private_key_path,
            key_passphrase,
        } => (private_key_path, key_passphrase),
        _ => panic!("expected private-key authentication"),
    };
    load_private_key_for_auth(&path, passphrase.as_deref())
}

#[test]
fn auth_requests_preserve_nonempty_passphrases() {
    for method in ["public_key", "auto"] {
        for passphrase in [
            None,
            Some(""),
            Some(" leading"),
            Some("trailing "),
            Some(" both "),
            Some(" \t "),
            Some("秘密🔑"),
            Some("\u{2003}秘密🔑\u{3000}"),
            Some("\u{2003}\u{3000}"),
        ] {
            let request = build_auth_request(
                Some(method.to_string()),
                String::new(),
                Some(" synthetic-key ".to_string()),
                passphrase.map(str::to_string),
                None,
            )
            .unwrap();
            let expected = passphrase
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            match request {
                SshAuthRequest::Auto {
                    private_key_path,
                    key_passphrase,
                } => {
                    assert_eq!(private_key_path.as_deref(), Some("synthetic-key"));
                    assert_eq!(key_passphrase, expected);
                }
                SshAuthRequest::PublicKey {
                    private_key_path,
                    key_passphrase,
                } => {
                    assert_eq!(private_key_path, "synthetic-key");
                    assert_eq!(key_passphrase, expected);
                }
                _ => panic!("expected private-key authentication"),
            }
        }
    }
}

#[test]
fn encrypted_keys_load_with_original_passphrases_only() {
    for passphrase in [
        " leading",
        "trailing ",
        " both ",
        "   ",
        "\u{2003}秘密🔑\u{3000}",
        "\u{2003}\u{3000}",
    ] {
        let key = TestKey::new(passphrase);
        for method in ["public_key", "auto"] {
            assert!(request_key(key.request(method, Some(passphrase))).is_ok());
            assert!(request_key(key.request(method, Some(passphrase.trim()))).is_err());
        }
    }
}

#[test]
fn unencrypted_keys_load_with_unspecified_passphrases() {
    let key = TestKey::new("");
    for method in ["public_key", "auto"] {
        for passphrase in [None, Some("")] {
            assert!(request_key(key.request(method, passphrase)).is_ok());
        }
    }
}

#[tokio::test]
async fn jump_and_target_preserve_independent_passphrases() {
    let jump_passphrase = " jump secret ";
    let target_passphrase = "\u{2003}接続先🔑\u{3000}";
    let jump_key = TestKey::new(jump_passphrase);
    let target_key = TestKey::new(target_passphrase);
    for jump_method in ["public_key", "auto"] {
        for target_method in ["public_key", "auto"] {
            let jump_requests = Arc::new(Mutex::new(Requests::default()));
            let target_requests = Arc::new(Mutex::new(Requests::default()));
            let mut jump = connect(
                TestServer {
                    accepted: "ssh-ed25519",
                    requests: Arc::clone(&jump_requests),
                    reject: false,
                    target: Some(Box::new(TestServer {
                        accepted: "ssh-ed25519",
                        requests: Arc::clone(&target_requests),
                        reject: false,
                        target: None,
                    })),
                },
                true,
            )
            .await;
            let key = request_key(jump_key.request(jump_method, Some(jump_passphrase))).unwrap();
            assert!(authenticate_public_key(&mut jump, "synthetic", key)
                .await
                .unwrap()
                .success());
            let channel = jump
                .channel_open_direct_tcpip("synthetic", 22, "synthetic", 0)
                .await
                .unwrap();
            let (observed, forwarded) = tokio::io::duplex(65536);
            let forwarder = tokio::spawn(async move {
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
            let key =
                request_key(target_key.request(target_method, Some(target_passphrase))).unwrap();
            assert!(authenticate_public_key(&mut target, "synthetic", key)
                .await
                .unwrap()
                .success());
            assert_eq!(jump_requests.lock().unwrap().signed, ["ssh-ed25519"]);
            assert_eq!(target_requests.lock().unwrap().signed, ["ssh-ed25519"]);
            forwarder.abort();
            let _ = forwarder.await;
        }
    }
}
