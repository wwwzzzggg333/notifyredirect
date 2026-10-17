use notifyredirect_core::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
type Callback = Arc<dyn Fn(Message) -> Result<()> + Send + Sync>;
pub struct ApiHandle {
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    port: u16,
}
impl ApiHandle {
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn stop(mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
impl Drop for ApiHandle {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
pub fn start(port: u16, token: String, callback: Callback) -> Result<ApiHandle> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|_| "本机入口端口被占用")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "无法配置本机入口")?;
    let port = listener.local_addr().map_err(|_| "地址错误")?.port();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let join = thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if count.load(Ordering::SeqCst) >= 8 {
                        drop(stream);
                        continue;
                    }
                    count.fetch_add(1, Ordering::SeqCst);
                    let count = count.clone();
                    let token = token.clone();
                    let cb = callback.clone();
                    let stopped = stop.clone();
                    thread::spawn(move || {
                        serve(stream, &token, cb, stopped);
                        count.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
    });
    Ok(ApiHandle {
        cancel,
        join: Some(join),
        port,
    })
}
fn read_exact(
    s: &mut TcpStream,
    buf: &mut [u8],
    deadline: Instant,
) -> std::result::Result<(), u16> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(400);
    }
    s.set_read_timeout(Some(left)).map_err(|_| 400u16)?;
    s.read_exact(buf).map_err(|_| 400u16)
}
fn line(s: &mut TcpStream, max: usize, deadline: Instant) -> std::result::Result<Vec<u8>, u16> {
    let mut v = vec![];
    loop {
        if v.len() >= max {
            return Err(413);
        }
        let mut b = [0];
        read_exact(s, &mut b, deadline)?;
        v.push(b[0]);
        if v.ends_with(b"\r\n") {
            v.truncate(v.len() - 2);
            return Ok(v);
        }
    }
}
fn request(s: &mut TcpStream, token: &str, deadline: Instant) -> std::result::Result<Vec<u8>, u16> {
    let mut head = vec![];
    loop {
        if head.len() >= 16384 {
            return Err(413);
        }
        let mut b = [0];
        read_exact(s, &mut b, deadline)?;
        head.push(b[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8(head).map_err(|_| 400u16)?;
    let mut lines = head.split("\r\n");
    if lines.next() != Some("POST /v1/events HTTP/1.1") {
        return Err(404);
    }
    let mut len = None;
    let mut chunked = false;
    let mut auth = None;
    let mut kind = None;
    for l in lines.filter(|l| !l.is_empty()) {
        let (k, v) = l.split_once(':').ok_or(400u16)?;
        let v = v.trim();
        match k.to_ascii_lowercase().as_str() {
            "content-length" => {
                if len.is_some() {
                    return Err(400);
                }
                len = Some(v.parse::<usize>().map_err(|_| 400u16)?);
            }
            "transfer-encoding" => {
                if chunked || !v.eq_ignore_ascii_case("chunked") {
                    return Err(400);
                }
                chunked = true
            }
            "authorization" => {
                if auth.replace(v).is_some() {
                    return Err(400);
                }
            }
            "content-type" if kind.replace(v).is_some() => {
                return Err(400);
            }
            _ => {}
        }
    }
    if !auth.is_some_and(|s| {
        s.strip_prefix("Bearer ")
            .is_some_and(|s| bool::from(s.as_bytes().ct_eq(token.as_bytes())))
    }) {
        return Err(401);
    }
    if !kind.is_some_and(|s| {
        s.split(';')
            .next()
            .unwrap_or("")
            .eq_ignore_ascii_case("application/json")
    }) {
        return Err(415);
    }
    if chunked && len.is_some() {
        return Err(400);
    }
    let mut body = vec![];
    if chunked {
        loop {
            let l = line(s, 128, deadline)?;
            let l = std::str::from_utf8(&l).map_err(|_| 400u16)?;
            let size =
                usize::from_str_radix(l.split(';').next().unwrap_or(""), 16).map_err(|_| 400u16)?;
            if size == 0 {
                if !line(s, 128, deadline)?.is_empty() {
                    return Err(400);
                }
                break;
            }
            if size > 131072 - body.len() {
                return Err(413);
            }
            let pos = body.len();
            body.resize(pos + size, 0);
            read_exact(s, &mut body[pos..], deadline)?;
            let mut end = [0; 2];
            read_exact(s, &mut end, deadline)?;
            if end != *b"\r\n" {
                return Err(400);
            }
        }
    } else {
        let len = len.ok_or(400u16)?;
        if len > 131072 {
            return Err(413);
        }
        body.resize(len, 0);
        read_exact(s, &mut body, deadline)?
    }
    Ok(body)
}
fn serve(mut stream: TcpStream, token: &str, callback: Callback, cancel: Arc<AtomicBool>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let response = match request(&mut stream, token, deadline) {
        Ok(body) => {
            if cancel.load(Ordering::SeqCst) {
                503
            } else {
                match parse_event(&body, chrono::Utc::now()) {
                    Ok(m) => {
                        if callback(m).is_ok() {
                            202
                        } else {
                            503
                        }
                    }
                    Err(_) => 400,
                }
            }
        }
        Err(code) => code,
    };
    let body = if response == 202 {
        r#"{"accepted":true,"delivery":"check_history"}"#
    } else {
        r#"{"accepted":false}"#
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = write!(
        stream,
        "HTTP/1.1 {response} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let drain_until = Instant::now() + Duration::from_millis(50);
    let mut drained = 0;
    let mut buffer = [0u8; 4096];
    while Instant::now() < drain_until && drained < 147456 {
        let _ =
            stream.set_read_timeout(Some(drain_until.saturating_duration_since(Instant::now())));
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
}
