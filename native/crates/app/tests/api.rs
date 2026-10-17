use notifyredirect_app::api::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn post(port: u16, token: &str, body: &str, kind: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(4)))
        .unwrap();
    write!(s,"POST /v1/events HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}
#[test]
fn auth_validation_and_body_limits() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let api = start(
        0,
        "token".into(),
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }),
    )
    .unwrap();
    let p = api.port();
    let valid = r#"{"version":1,"event_id":"e","source":"agent","title":"Done"}"#;
    assert!(post(p, "bad", valid, "application/json").contains("401"));
    assert!(post(p, "token", "{}", "application/json").contains("400"));
    assert!(post(p, "token", valid, "text/plain").contains("415"));
    assert!(post(p, "token", valid, "application/json").contains("202"));
    assert!(post(p, "token", &"x".repeat(131073), "application/json").contains("413"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    api.stop();
    assert!(TcpStream::connect(("127.0.0.1", p)).is_err());
}

fn raw_post(port: u16, head: &str, body: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    write!(s,"POST /v1/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer token\r\nContent-Type: application/json\r\n{head}\r\n{body}").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}
#[test]
fn rejects_ambiguous_framing_and_accepts_bounded_chunks() {
    let api = start(0, "token".into(), Arc::new(|_| Ok(()))).unwrap();
    let p = api.port();
    assert!(raw_post(p, "Content-Length: 2\r\nContent-Length: 2\r\n", "{}").contains("400"));
    assert!(
        raw_post(
            p,
            "Transfer-Encoding: chunked\r\nContent-Length: 2\r\n",
            "{}"
        )
        .contains("400")
    );
    let body = r#"{"version":1,"event_id":"e","source":"a","title":"T"}"#;
    let chunk = format!("{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body);
    assert!(raw_post(p, "Transfer-Encoding: chunked\r\n", &chunk).contains("202"));
    assert!(raw_post(p, "Transfer-Encoding: chunked\r\n", "20001\r\n").contains("413"));
    api.stop();
}
