//! A tiny local HTTP server for tests. Each connection gets one scripted reply.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub delay: Duration,
}

impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
            delay: Duration::ZERO,
        }
    }
    pub fn redirect(to: &str) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".into(), to.into())],
            body: vec![],
            delay: Duration::ZERO,
        }
    }
    pub fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            headers: vec![],
            body,
            delay: Duration::ZERO,
        }
    }
}

pub struct Server {
    pub base: String,
    pub requests: Arc<Mutex<Vec<String>>>,
}

/// Serve `handler(request_text)` for every connection, on a background thread.
pub fn serve(handler: impl Fn(&str) -> Reply + Send + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut data = Vec::new();
            let mut buf = [0u8; 4096];
            s.set_read_timeout(Some(Duration::from_secs(2))).ok();
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..pos]).to_lowercase();
                    let want = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if data.len() >= pos + 4 + want {
                        break;
                    }
                }
            }
            let text = String::from_utf8_lossy(&data).into_owned();
            log.lock().unwrap().push(text.clone());
            let r = handler(&text);
            std::thread::sleep(r.delay);
            let mut head = format!(
                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                r.status,
                r.body.len()
            );
            for (k, v) in &r.headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&r.body);
        }
    });
    Server { base, requests }
}
