// src-tauri/src/loopback.rs
//
// RFC 8252 loopback redirect for the browser sign-in. Debug builds on macOS
// use it instead of `notefix://auth`: the bare `tauri dev` binary is no app
// bundle, so LaunchServices hands `notefix://` links to the installed app
// (or to nobody). The server ignores the port of a loopback redirect, so we
// listen on any free one for a single `/auth` request and turn it back into
// the `notefix://auth?…` URL the normal completion path already handles.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Whether this build signs in through the loopback listener.
pub const ENABLED: bool = cfg!(all(debug_assertions, target_os = "macos"));

/// How long a started sign-in waits for the browser to come back.
const WAIT: Duration = Duration::from_secs(600);

const MAX_REQUEST: usize = 8 * 1024;

/// Map the request line of the browser's redirect (`GET /auth?code=… HTTP/1.1`)
/// to the deep-link URL `server_auth_complete` parses. Anything else — a
/// favicon request, another path — is `None`.
pub fn callback_url(request_line: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    (path == "/auth").then(|| format!("notefix://auth?{query}"))
}

fn response(ok: bool) -> String {
    let (status, body) = if ok {
        (
            "200 OK",
            "<!doctype html><meta charset=utf-8><title>Notefix</title>\
             <p>Signed in — you can close this tab and return to Notefix.</p>",
        )
    } else {
        ("404 Not Found", "")
    };
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Read one request's head and answer it; the callback URL if it was `/auth`.
async fn serve(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let url = head.lines().next().and_then(callback_url);
    let _ = stream.write_all(response(url.is_some()).as_bytes()).await;
    let _ = stream.shutdown().await;
    url
}

async fn accept_callback(listener: TcpListener) -> Option<String> {
    loop {
        let (mut stream, _) = listener.accept().await.ok()?;
        if let Some(url) = serve(&mut stream).await {
            return Some(url);
        }
    }
}

/// Bind a free loopback port. Returns the `redirect_uri` to send to the server
/// and a future resolving to the callback URL (or `None` after the timeout).
pub async fn listen() -> Result<(String, impl std::future::Future<Output = Option<String>>), String>
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let wait = async move {
        tokio::time::timeout(WAIT, accept_callback(listener))
            .await
            .ok()
            .flatten()
    };
    Ok((format!("http://127.0.0.1:{port}/auth"), wait))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_url_keeps_the_query() {
        assert_eq!(
            callback_url("GET /auth?code=abc&state=xyz HTTP/1.1").as_deref(),
            Some("notefix://auth?code=abc&state=xyz")
        );
    }

    #[test]
    fn callback_url_ignores_other_paths_and_methods() {
        assert_eq!(callback_url("GET /favicon.ico HTTP/1.1"), None);
        assert_eq!(callback_url("POST /auth?code=a HTTP/1.1"), None);
        assert_eq!(callback_url(""), None);
    }

    #[tokio::test]
    async fn listen_answers_stray_requests_and_returns_the_auth_callback() {
        let (redirect, wait) = listen().await.unwrap();
        let addr = redirect
            .trim_start_matches("http://")
            .trim_end_matches("/auth")
            .to_string();
        let waiter = tokio::spawn(wait);

        let request = |path: &'static str| {
            let addr = addr.clone();
            async move {
                let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
                s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
                    .await
                    .unwrap();
                let mut out = String::new();
                s.read_to_string(&mut out).await.unwrap();
                out
            }
        };

        assert!(request("/favicon.ico").await.starts_with("HTTP/1.1 404"));
        assert!(request("/auth?code=c1&state=s1").await.starts_with("HTTP/1.1 200"));
        assert_eq!(
            waiter.await.unwrap().as_deref(),
            Some("notefix://auth?code=c1&state=s1")
        );
    }
}
