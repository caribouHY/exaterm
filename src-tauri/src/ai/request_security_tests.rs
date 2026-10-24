use super::*;
use crate::ai::debug_log::{
    append_chat_debug_log_if_enabled, append_chat_debug_log_to_dir, ChatDebugLogInput,
    ChatDebugLogOutcome,
};
use crate::command_error::BackendCommandError;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

const FAKE_KEY: &str = "FAKE-EXATERM-SECURITY-TEST-KEY-NOT-A-CREDENTIAL";

fn client() -> reqwest::Client {
    ai_http_client_builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

fn refused_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}/models", listener.local_addr().unwrap())
}

fn serve(response: String) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/models", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        while !request.windows(4).any(|part| part == b"\r\n\r\n") {
            let read = socket.read(&mut buffer).unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&buffer[..read]);
        }
        socket.write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, thread)
}

fn response(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn execute(method: Method, url: &str, key: &str) -> Result<String, String> {
    let request = gemini_request(&client(), method.clone(), url, key);
    if method == Method::POST {
        let body = send_chat_request_json(request, &AiProvider::Gemini, "en").await?;
        extract_chat_text(&body, &AiProvider::Gemini, "en", ChatResponseFormat::Gemini)
    } else {
        let body = send_model_list_request(request, &AiProvider::Gemini, "en").await?;
        extract_models(&body, &AiProvider::Gemini, "en").map(|models| models[0].model_id.clone())
    }
}

fn assert_safe_error(error: &str, expected: &str) {
    assert!(error.contains("Gemini"), "{error}");
    assert!(error.contains(expected), "{error}");
    let payload = serde_json::to_string(&BackendCommandError::from(error)).unwrap();
    for text in [error, payload.as_str()] {
        assert!(!text.contains(FAKE_KEY));
        assert!(!text.contains("?key="));
        assert!(!text.contains("http://"));
    }

    for enabled in [false, true] {
        let dir =
            std::env::temp_dir().join(format!("exaterm_ai_security_{}", uuid::Uuid::new_v4()));
        let mut written = false;
        append_chat_debug_log_if_enabled(
            enabled,
            ChatDebugLogInput {
                provider: &AiProvider::Gemini,
                model: "synthetic-model",
                language: "en",
                terminal_context_included: false,
                messages: &[],
                system_prompt: "synthetic prompt",
                duration_ms: 1,
                outcome: ChatDebugLogOutcome::Error { error },
            },
            |input| {
                written = true;
                let path = append_chat_debug_log_to_dir(&dir, input)?;
                let data = std::fs::read_to_string(path).unwrap();
                let record: Value = serde_json::from_str(data.trim()).unwrap();
                assert_eq!(record["error"], error);
                assert_eq!(record["status"], "error");
                assert!(!data.contains(FAKE_KEY));
                assert!(!data.contains("?key="));
                assert!(!data.contains("http://"));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(written, enabled);
        assert_eq!(dir.exists(), enabled);
        if enabled {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn gemini_authentication_is_a_sensitive_header_for_both_operations() {
    for (method, url) in [
        (Method::GET, GEMINI_MODELS_URL.to_string()),
        (
            Method::POST,
            format!("{GEMINI_GENERATE_BASE_URL}/synthetic:generateContent"),
        ),
    ] {
        let request = gemini_request(&client(), method.clone(), &url, FAKE_KEY)
            .build()
            .unwrap();
        assert_eq!(request.method(), method);
        assert_eq!(request.url().as_str(), url);
        assert!(request.url().query().is_none());
        let header = &request.headers()["x-goog-api-key"];
        assert_eq!(header, FAKE_KEY);
        assert!(header.is_sensitive());
        assert!(!format!("{request:?}").contains(FAKE_KEY));
    }
}

#[tokio::test]
async fn refused_connections_do_not_publish_legacy_key_urls() {
    for method in [Method::GET, Method::POST] {
        let url = format!("{}?key={FAKE_KEY}", refused_url());
        let error = execute(method, &url, FAKE_KEY).await.unwrap_err();
        assert_safe_error(&error, "connection failed");
        assert_eq!(
            BackendCommandError::from(error).code,
            "ai.provider_unavailable"
        );
    }
}

#[tokio::test]
async fn http_errors_never_publish_echoed_credentials_or_urls() {
    for method in [Method::GET, Method::POST] {
        for status in [400, 401, 403, 404, 429, 500, 503] {
            for body in [
                format!(
                    r#"{{"error":{{"message":"{FAKE_KEY} http://localhost/?key={FAKE_KEY}"}}}}"#
                ),
                format!("proxy rejected http://localhost/?key={FAKE_KEY}"),
            ] {
                let (url, server) = serve(response(status, &body));
                let error = execute(method.clone(), &url, FAKE_KEY).await.unwrap_err();
                let request = server.join().unwrap();
                assert!(!request.lines().next().unwrap().contains(FAKE_KEY));
                assert!(request.contains(&format!("x-goog-api-key: {FAKE_KEY}")));
                assert_safe_error(&error, &format!("HTTP {status}"));
                let payload = BackendCommandError::from(error);
                let code = match status {
                    401 | 403 => "ai.authentication_failed",
                    404 => "ai.model_not_found",
                    429 => "ai.quota_exceeded",
                    500..=599 => "ai.server_error",
                    _ => "ai.http_error",
                };
                assert_eq!(payload.code, code);
            }
        }
    }
}

#[tokio::test]
async fn malformed_responses_do_not_publish_response_bodies() {
    for method in [Method::GET, Method::POST] {
        for body in [
            format!("not json: {FAKE_KEY} http://localhost/?key={FAKE_KEY}"),
            format!(r#"{{"error":{{"message":"{FAKE_KEY}"}}}}"#),
        ] {
            let (url, server) = serve(response(200, &body));
            let error = execute(method.clone(), &url, FAKE_KEY).await.unwrap_err();
            server.join().unwrap();
            assert_safe_error(&error, "response");
        }
    }
}

#[tokio::test]
async fn response_body_failures_do_not_publish_request_urls() {
    for method in [Method::GET, Method::POST] {
        let body = format!("{FAKE_KEY} http://localhost/?key={FAKE_KEY}");
        let (url, server) = serve(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: 99999\r\nConnection: close\r\n\r\n{body}"
        ));
        let error = execute(method, &format!("{url}?key={FAKE_KEY}"), FAKE_KEY)
            .await
            .unwrap_err();
        server.join().unwrap();
        assert_safe_error(&error, "response decoding failed");
    }
}

#[tokio::test]
async fn invalid_authentication_headers_return_safe_builder_errors() {
    for method in [Method::GET, Method::POST] {
        let error = execute(method, &refused_url(), &format!("{FAKE_KEY}\ninvalid"))
            .await
            .unwrap_err();
        assert_safe_error(&error, "invalid request configuration");
    }
}

#[tokio::test]
async fn valid_chat_and_models_responses_still_parse() {
    for (method, body, expected) in [
        (
            Method::POST,
            r#"{"candidates":[{"content":{"parts":[{"text":"synthetic reply"}]}}]}"#,
            "synthetic reply",
        ),
        (
            Method::GET,
            r#"{"models":[{"name":"models/synthetic-model","supportedGenerationMethods":["generateContent"]}]}"#,
            "synthetic-model",
        ),
    ] {
        let (url, server) = serve(response(200, body));
        assert_eq!(execute(method, &url, FAKE_KEY).await.unwrap(), expected);
        server.join().unwrap();
    }
}

#[tokio::test]
async fn redirects_cannot_forward_authentication_to_another_origin() {
    for method in [Method::GET, Method::POST] {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let target = format!(
            "http://{}/?key={FAKE_KEY}",
            destination.local_addr().unwrap()
        );
        let (url, server) = serve(format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ));
        let error = execute(method, &url, FAKE_KEY).await.unwrap_err();
        server.join().unwrap();
        assert_safe_error(&error, "HTTP 307");
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
