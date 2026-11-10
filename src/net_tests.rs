use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Serves one canned response to the first connection and returns the request it read.
fn serve(response: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/pic.png", listener.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut req = Vec::new();
        let mut buf = [0u8; 1024];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = sock.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            req.extend_from_slice(&buf[..n]);
        }
        let _ = sock.write_all(&response);
        String::from_utf8_lossy(&req).into_owned()
    });
    (url, h)
}

fn ok(body: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    r.extend_from_slice(body);
    r
}

#[test]
fn only_http_and_https_are_allowed() {
    assert!(check_url("https://h.example/x.png").is_ok());
    assert!(check_url("HTTP://h.example/x.png").is_ok());
    assert!(check_url("file:///etc/passwd").is_err());
    assert!(check_url("ftp://h.example/x.png").is_err());
    assert!(check_url("data:image/png;base64,AA").is_err());
    assert!(check_url("x.png").is_err());
    assert!(check_url("").is_err());
}

#[test]
fn fetch_returns_the_body_and_names_itself() {
    let (url, server) = serve(ok(b"hello"));
    assert_eq!(fetch(&url).unwrap(), b"hello");
    let req = server.join().unwrap().to_ascii_lowercase();
    assert!(req.contains("user-agent: rune-markdown"), "{req}");
}

#[test]
fn fetch_refuses_a_body_over_the_cap() {
    let (url, _server) = serve(ok(&[7u8; 100]));
    let err = fetch_with(&url, 50, TIMEOUT).unwrap_err();
    assert!(err.contains("too large"), "{err}");
}

#[test]
fn fetch_caps_a_body_with_no_length() {
    let mut r = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    r.extend_from_slice(&[7u8; 100]);
    let (url, _server) = serve(r);
    let err = fetch_with(&url, 50, TIMEOUT).unwrap_err();
    assert!(err.contains("too large"), "{err}");
}

#[test]
fn fetch_reports_an_http_error() {
    let (url, _server) = serve(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
    assert!(fetch(&url).is_err());
}

#[test]
fn fetch_rejects_other_schemes_without_connecting() {
    assert!(fetch("file:///etc/passwd").is_err());
}
