use notifyredirect_platform::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
fn request(response: Vec<u8>, limit: usize) -> std::result::Result<HttpResponse, HttpError> {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let thread = std::thread::spawn(move || {
        let (mut s, _) = server.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut head = vec![];
        while !head.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            if s.read_exact(&mut b).is_err() {
                return;
            }
            head.push(b[0]);
        }
        let mut body = [0; 2];
        let _ = s.read_exact(&mut body);
        let _ = s.write_all(&response);
        let _ = s.shutdown(std::net::Shutdown::Write);
    });
    let result = WinHttpTransport.send(&HttpRequest {
        url: format!("http://127.0.0.1:{port}/test"),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: b"{}".to_vec(),
        timeout: Duration::from_secs(3),
        max_response_bytes: limit,
    });
    thread.join().unwrap();
    result
}
#[test]
fn does_not_follow_redirects_and_bounds_response() {
    let r=request(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),65536).unwrap();
    assert_eq!(r.status, 302);
    let mut response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n".to_vec();
    response.extend(vec![b'x'; 65537]);
    assert!(matches!(
        request(response, 65536),
        Err(HttpError::ResponseTooLarge)
    ));
}
#[test]
fn ambiguous_connections_and_slow_responses_obey_deadline() {
    let s = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = s.local_addr().unwrap().port();
    drop(s);
    let start = Instant::now();
    let result = WinHttpTransport.send(&HttpRequest {
        url: format!("http://127.0.0.1:{port}/test"),
        headers: vec![],
        body: vec![],
        timeout: Duration::from_millis(100),
        max_response_bytes: 64,
    });
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_millis(400));
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let t = std::thread::spawn(move || {
        let (_s, _) = server.accept().unwrap();
        std::thread::sleep(Duration::from_millis(500));
    });
    let start = Instant::now();
    let r = WinHttpTransport.send(&HttpRequest {
        url: format!("http://127.0.0.1:{port}/test"),
        headers: vec![],
        body: vec![],
        timeout: Duration::from_millis(100),
        max_response_bytes: 64,
    });
    assert!(matches!(r, Err(HttpError::Uncertain)));
    assert!(start.elapsed() < Duration::from_millis(400));
    t.join().unwrap();
}
