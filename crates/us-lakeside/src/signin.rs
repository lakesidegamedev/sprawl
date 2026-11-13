//! Optional in-game sign-in: loopback listener plus system browser, one-time code exchanged for
//! a 30 day session token kept in the OS secure store (or memory only).

use crate::{Config, FALLBACK_NAME, clean_name};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const PORTS: std::ops::RangeInclusive<u16> = 47832..=47841;
pub(crate) const LIFETIME_SECS: u64 = 30 * 24 * 60 * 60;
const MAX_BODY: u64 = 16 * 1024;

/// How a sign-in ended. Reasons are plain words for the player and never contain the token or code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInResult {
    Done { name: String },
    Failed(String),
    Cancelled,
}

/// The session kept between runs. Never printed.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Saved {
    pub token: String,
    pub name: String,
    #[serde(default)]
    pub id: String,
    pub saved_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn expired(s: &Saved) -> bool {
    now_secs().saturating_sub(s.saved_at) > LIFETIME_SECS
}

/// Where the sign-in secret lives between runs: the OS secure store, never a plain file.
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Option<String>;
    /// False when this machine has no working secure store; the sign-in then lasts this session only.
    fn save(&self, secret: &str) -> bool;
    fn delete(&self);
}

/// Windows Credential Manager, macOS Keychain, Secret Service on Linux.
pub struct KeyringStore {
    service: &'static str,
    user: &'static str,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: "net.lakesidegames.urban-sprawl",
            user: "lakeside-session",
        }
    }
    fn entry(&self) -> Option<keyring::Entry> {
        keyring::Entry::new(self.service, self.user).ok()
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for KeyringStore {
    fn load(&self) -> Option<String> {
        self.entry()?.get_password().ok()
    }
    fn save(&self, secret: &str) -> bool {
        self.entry().is_some_and(|e| e.set_password(secret).is_ok())
    }
    fn delete(&self) {
        if let Some(e) = self.entry() {
            let _ = e.delete_credential();
        }
    }
}

/// Keeps nothing past the process. Also the test store.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl TokenStore for MemoryStore {
    fn load(&self) -> Option<String> {
        self.0.lock().ok()?.clone()
    }
    fn save(&self, secret: &str) -> bool {
        if let Ok(mut g) = self.0.lock() {
            *g = Some(secret.to_string());
        }
        true
    }
    fn delete(&self) {
        if let Ok(mut g) = self.0.lock() {
            *g = None;
        }
    }
}

pub(crate) fn load(store: &dyn TokenStore) -> Option<Saved> {
    let text = store.load()?;
    let saved: Saved = serde_json::from_str(&text).ok()?;
    if saved.token.trim().is_empty() || expired(&saved) {
        store.delete();
        return None;
    }
    Some(saved)
}

pub(crate) fn save(store: &dyn TokenStore, saved: &Saved) {
    if let Ok(text) = serde_json::to_string(saved) {
        // No working secure store: the sign-in lasts this session only. That is the promise.
        let _ = store.save(&text);
    }
}

/// Open a URL in the system browser. False when it could not be started.
pub(crate) fn open_in_browser(url: &str) -> bool {
    use std::process::{Command, Stdio};
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn random_state() -> Option<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(
        bytes
            .iter()
            .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
            .collect(),
    )
}

fn bind(ports: &std::ops::RangeInclusive<u16>) -> Option<TcpListener> {
    ports
        .clone()
        .find_map(|p| TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, p))).ok())
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(v) => {
                    out.push(v);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn reply(stream: &mut TcpStream, status: u16, text: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>Urban Sprawl</title><body style=\"font-family:sans-serif;background:#111;color:#eee;text-align:center;padding-top:20vh\"><p>{text}</p>"
    );
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        if status == 200 { "OK" } else { "Error" },
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn request_line(stream: &mut TcpStream) -> Option<String> {
    let mut out = Vec::new();
    let mut byte = [0u8; 1];
    while out.len() < 4096 {
        if stream.read(&mut byte).ok()? == 0 || byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            out.push(byte[0]);
        }
    }
    String::from_utf8(out).ok()
}

enum Wait {
    Code(String),
    Cancelled,
    TimedOut,
}

fn await_code(listener: &TcpListener, state: &str, wait: Duration, cancel: &AtomicBool) -> Wait {
    let end = Instant::now() + wait;
    listener.set_nonblocking(true).ok();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Wait::Cancelled;
        }
        if Instant::now() >= end {
            return Wait::TimedOut;
        }
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        stream.set_nonblocking(false).ok();
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let line = request_line(&mut stream).unwrap_or_default();
        let target = line.split(' ').nth(1).unwrap_or("").to_string();
        let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
        if path != "/callback" {
            reply(&mut stream, 404, "Not found.");
            continue;
        }
        let mut code = String::new();
        let mut got_state = String::new();
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                match decode(k).as_str() {
                    "code" => code = decode(v),
                    "state" => got_state = decode(v),
                    _ => {}
                }
            }
        }
        if got_state != state || code.trim().is_empty() || code.len() > 2048 {
            reply(
                &mut stream,
                400,
                "This sign-in link does not match. Go back to Urban Sprawl and try again.",
            );
            continue;
        }
        reply(
            &mut stream,
            200,
            "You are signed in to Lakeside. You can close this window and go back to Urban Sprawl.",
        );
        return Wait::Code(code);
    }
}

pub(crate) fn run(cfg: &Config, cancel: &AtomicBool) -> Result<Saved, SignInResult> {
    let Some(listener) = bind(&cfg.ports) else {
        return Err(SignInResult::Failed(
            "Could not open a local port for the sign-in. Close other Lakeside apps and try again."
                .into(),
        ));
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let Some(state) = random_state() else {
        return Err(SignInResult::Failed(
            "Could not start a safe sign-in on this computer.".into(),
        ));
    };
    // The same query CW sends: provider, loopback port, random state.
    let url = format!(
        "{}/account/launcher/start?provider=ahd&port={port}&state={state}",
        cfg.base
    );
    if !(cfg.open_url)(&url) {
        return Err(SignInResult::Failed(format!(
            "Could not open your browser. Open {} and sign in there, or try again.",
            crate::STORE_URL
        )));
    }
    let code = match await_code(&listener, &state, cfg.sign_in_wait, cancel) {
        Wait::Code(c) => c,
        Wait::Cancelled => return Err(SignInResult::Cancelled),
        Wait::TimedOut => {
            return Err(SignInResult::Failed(
                "The sign-in was not finished in time. Try again.".into(),
            ));
        }
    };
    drop(listener);
    exchange(cfg, &code)
}

#[derive(Deserialize)]
struct SessionInfo {
    username: Option<String>,
    #[serde(rename = "accountId")]
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct SessionReply {
    token: Option<String>,
    session: Option<SessionInfo>,
}

/// One attempt, no retry: a code is single use, and 429 means wait.
fn exchange(cfg: &Config, code: &str) -> Result<Saved, SignInResult> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .redirects(0)
        .build();
    let url = format!("{}/account/launcher/session", cfg.base);
    let failed = |s: &str| Err(SignInResult::Failed(s.to_string()));
    match agent
        .post(&url)
        .set("Accept", "application/json")
        .send_json(serde_json::json!({ "code": code }))
    {
        Ok(resp) if resp.status() == 200 => {
            let mut bytes = Vec::new();
            if resp
                .into_reader()
                .take(MAX_BODY + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() as u64 > MAX_BODY
            {
                return failed("Lakeside sent back something unexpected. Try again.");
            }
            let Ok(reply) = serde_json::from_slice::<SessionReply>(&bytes) else {
                return failed("Lakeside sent back something unexpected. Try again.");
            };
            let token = reply.token.unwrap_or_default().trim().to_string();
            if token.is_empty() {
                return failed("Lakeside sent back something unexpected. Try again.");
            }
            let (raw_name, id) = reply
                .session
                .map(|s| {
                    (
                        s.username.unwrap_or_default(),
                        s.account_id.unwrap_or_default(),
                    )
                })
                .unwrap_or_default();
            let name = clean_name(&raw_name);
            Ok(Saved {
                token,
                name: if name.is_empty() {
                    FALLBACK_NAME.into()
                } else {
                    name
                },
                id: id.trim().chars().take(128).collect(),
                saved_at: now_secs(),
            })
        }
        Ok(resp) => failed(&format!(
            "Lakeside sign-in failed (error {}). Try again later.",
            resp.status()
        )),
        Err(ureq::Error::Status(400, _)) => failed("That sign-in link expired. Try again."),
        Err(ureq::Error::Status(401, _)) => {
            failed("Lakeside did not accept the sign-in. Try again.")
        }
        Err(ureq::Error::Status(429, _)) => {
            failed("Too many sign-in attempts. Wait a few minutes and try again.")
        }
        Err(ureq::Error::Status(502 | 503, _)) => {
            failed("Lakeside sign-in is not available right now. Try again later.")
        }
        Err(ureq::Error::Status(c, _)) => failed(&format!(
            "Lakeside sign-in failed (error {c}). Try again later."
        )),
        Err(_) => failed("Could not reach Lakeside to finish signing in."),
    }
}
