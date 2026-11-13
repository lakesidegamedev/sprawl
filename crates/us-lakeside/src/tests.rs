use crate::mock::{Reply, serve};
use crate::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SECRET: &str = "SECRET-HANDOFF-CODE-9f8e7d";

fn cfg(base: &str) -> Config {
    Config {
        base: base.into(),
        current_version: "1.0.0-preview.1".into(),
        handoff_timeout: Duration::from_secs(1),
        ports: 47900..=47909,
        sign_in_wait: Duration::from_secs(10),
        open_url: |_| true,
        store: Arc::new(MemoryStore::default()),
    }
}

fn launcher_env(code: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
    move |k| match k {
        "LAKESIDE_LAUNCHER" => Some("1".into()),
        "LAKESIDE_LAUNCHER_VERSION" => Some("0.4.2".into()),
        "LAKESIDE_HANDOFF_CODE" => code.map(String::from),
        _ => None,
    }
}

fn wait_done(l: &Lakeside) -> Status {
    for _ in 0..200 {
        let s = l.status();
        if !s.handoff_pending {
            // restore thread may still be running; give it a moment
            std::thread::sleep(Duration::from_millis(30));
            return l.status();
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("handoff never finished");
}

fn redeem_with(reply: Reply) -> Option<Account> {
    let reply = Mutex::new(Some(reply));
    let srv = serve(move |_| reply.lock().unwrap().take().unwrap_or(Reply::json(500, "")));
    handoff::redeem(
        &srv.base,
        handoff::HandoffCode::new(SECRET.into()),
        Duration::from_secs(1),
    )
}

#[test]
fn handoff_success_posts_code_and_client_id() {
    let srv = serve(|_| {
        Reply::json(
            200,
            r#"{"accountId":"acc_1","displayName":"  Ada\u0007 Lovelace  ","entitlements":[]}"#,
        )
    });
    let l = Lakeside::start_with(cfg(&srv.base), launcher_env(Some(SECRET)));
    let s = wait_done(&l);
    assert!(s.launcher);
    assert_eq!(s.launcher_version, "0.4.2");
    assert!(!s.updater_enabled);
    assert_eq!(
        s.account,
        Some(Account {
            id: "acc_1".into(),
            name: "Ada Lovelace".into()
        })
    );
    let reqs = srv.requests.lock().unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(reqs[0].starts_with("POST /account/launcher/handoff/redeem"));
    assert!(reqs[0].contains(r#""clientId":"urban-sprawl-desktop""#));
    assert!(reqs[0].contains(SECRET));
}

#[test]
fn handoff_failures_mean_not_signed_in() {
    assert!(redeem_with(Reply::json(400, r#"{"error":"expired"}"#)).is_none());
    assert!(redeem_with(Reply::json(401, "{}")).is_none());
    assert!(redeem_with(Reply::json(429, "{}")).is_none());
    assert!(redeem_with(Reply::json(500, "{}")).is_none());
    assert!(redeem_with(Reply::json(503, "{}")).is_none());
    assert!(redeem_with(Reply::json(200, "not json")).is_none());
    assert!(redeem_with(Reply::json(200, r#"{"displayName":"x"}"#)).is_none());
    assert!(redeem_with(Reply::json(200, r#"{"accountId":"   "}"#)).is_none());
    let huge = format!(
        r#"{{"accountId":"a","displayName":"{}"}}"#,
        "x".repeat(20_000)
    );
    assert!(redeem_with(Reply::json(200, &huge)).is_none());
}

#[test]
fn handoff_slow_reply_times_out() {
    let mut r = Reply::json(200, r#"{"accountId":"a","displayName":"n"}"#);
    r.delay = Duration::from_secs(5);
    let t = Instant::now();
    assert!(redeem_with(r).is_none());
    assert!(
        t.elapsed() < Duration::from_secs(3),
        "took {:?}",
        t.elapsed()
    );
}

#[test]
fn handoff_limit_is_five_seconds() {
    assert_eq!(HANDOFF_TIMEOUT, Duration::from_secs(5));
    assert_eq!(
        Config::production("1.0.0").handoff_timeout,
        Duration::from_secs(5)
    );
}

#[test]
fn handoff_redirect_is_not_followed() {
    let target = serve(|_| Reply::json(200, r#"{"accountId":"a","displayName":"n"}"#));
    let to = format!("{}/elsewhere", target.base);
    let srv = serve(move |_| Reply::redirect(&to));
    assert!(
        handoff::redeem(
            &srv.base,
            handoff::HandoffCode::new(SECRET.into()),
            Duration::from_secs(1)
        )
        .is_none()
    );
    assert!(
        target.requests.lock().unwrap().is_empty(),
        "redirect target was contacted"
    );
}

#[test]
fn handoff_is_never_retried() {
    let srv = serve(|_| Reply::json(500, "{}"));
    let l = Lakeside::start_with(cfg(&srv.base), launcher_env(Some(SECRET)));
    let s = wait_done(&l);
    assert!(s.account.is_none());
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(srv.requests.lock().unwrap().len(), 1);
}

#[test]
fn no_code_without_launcher_flag() {
    let srv = serve(|_| Reply::json(200, r#"{"accountId":"a","displayName":"n"}"#));
    let env = |k: &str| match k {
        "LAKESIDE_HANDOFF_CODE" => Some(SECRET.to_string()),
        "LAKESIDE_LAUNCHER" => Some("0".to_string()),
        _ => None,
    };
    let l = Lakeside::start_with(cfg(&srv.base), env);
    std::thread::sleep(Duration::from_millis(300));
    let s = l.status();
    assert!(
        !s.launcher && s.updater_enabled && s.account.is_none() && s.launcher_version.is_empty()
    );
    assert!(srv.requests.lock().unwrap().is_empty());
}

#[test]
fn launcher_without_code_makes_no_call() {
    let srv = serve(|_| Reply::json(200, "{}"));
    let l = Lakeside::start_with(cfg(&srv.base), launcher_env(None));
    std::thread::sleep(Duration::from_millis(300));
    assert!(l.status().launcher && l.status().account.is_none());
    assert!(srv.requests.lock().unwrap().is_empty());
}

#[test]
fn names_are_cleaned() {
    assert_eq!(clean_name("  hi\u{0}\nthere \t"), "hithere");
    assert_eq!(clean_name(&"a".repeat(80)).chars().count(), 40);
    let a = handoff::parse(br#"{"accountId":"x","displayName":""}"#).unwrap();
    assert_eq!(a.name, "your Lakeside account");
}

#[test]
fn code_never_appears_in_debug_output() {
    let srv = serve(|_| Reply::json(200, r#"{"accountId":"acc","displayName":"Ada"}"#));
    let l = Lakeside::start_with(cfg(&srv.base), launcher_env(Some(SECRET)));
    let s = wait_done(&l);
    for text in [
        format!("{l:?}"),
        format!("{s:?}"),
        format!("{:?}", s.account),
        format!("{:?}", handoff::HandoffCode::new(SECRET.into())),
    ] {
        assert!(!text.contains(SECRET), "{text}");
    }
    // Status has no field that could carry the code: exactly these five.
    let Status {
        launcher: _,
        launcher_version: _,
        account: _,
        updater_enabled: _,
        handoff_pending: _,
    } = s;
}

// ---- sign-in ----

static URL_A: Mutex<Option<String>> = Mutex::new(None);
static URL_B: Mutex<Option<String>> = Mutex::new(None);

fn param(url: &str, key: &str) -> String {
    url.split(['?', '&'])
        .find_map(|p| p.strip_prefix(&format!("{key}=")))
        .unwrap_or("")
        .to_string()
}

fn hit(port: &str, path_query: &str) -> String {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    write!(s, "GET {path_query} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).ok();
    out
}

#[test]
fn sign_in_flow_stores_token_and_sign_out_clears_it() {
    let srv = serve(|req| {
        if req.contains("\"code\":\"one-time\"") {
            Reply::json(
                200,
                r#"{"token":"tok-123","session":{"username":"Grace","accountId":"acc_9"}}"#,
            )
        } else {
            Reply::json(401, "{}")
        }
    });
    let store = Arc::new(MemoryStore::default());
    let mut c = cfg(&srv.base);
    c.store = store.clone();
    c.open_url = |u| {
        *URL_A.lock().unwrap() = Some(u.to_string());
        true
    };
    let l = Lakeside::start_with(c, |_| None);
    let l2 = l.clone();
    let t = std::thread::spawn(move || l2.sign_in());
    let url = loop {
        if let Some(u) = URL_A.lock().unwrap().clone() {
            break u;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(url.starts_with(&format!(
        "{}/account/launcher/start?provider=ahd&port=",
        srv.base
    )));
    let (port, state) = (param(&url, "port"), param(&url, "state"));
    assert_eq!(state.len(), 32);
    assert!(hit(&port, "/other").contains("404"));
    assert!(hit(&port, "/callback?code=one-time&state=wrong").contains("400"));
    assert!(hit(&port, &format!("/callback?code=one-time&state={state}")).contains("200 OK"));
    assert_eq!(
        t.join().unwrap(),
        SignInResult::Done {
            name: "Grace".into()
        }
    );
    assert_eq!(
        l.status().account,
        Some(Account {
            id: "acc_9".into(),
            name: "Grace".into()
        })
    );
    assert!(store.load().unwrap().contains("tok-123"));
    assert!(!format!("{:?}{:?}", l, l.status()).contains("tok-123"));
    l.sign_out();
    assert!(l.status().account.is_none());
    assert!(store.load().is_none());
}

#[test]
fn sign_in_cancel_and_failures() {
    let srv = serve(|_| Reply::json(429, "{}"));
    let mut c = cfg(&srv.base);
    c.open_url = |u| {
        *URL_B.lock().unwrap() = Some(u.to_string());
        true
    };
    let l = Lakeside::start_with(c, |_| None);
    let l2 = l.clone();
    let t = std::thread::spawn(move || l2.sign_in());
    while URL_B.lock().unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(20));
    }
    l.cancel_sign_in();
    assert_eq!(t.join().unwrap(), SignInResult::Cancelled);
    assert!(l.status().account.is_none());

    let mut c = cfg(&srv.base);
    c.open_url = |_| false;
    let l = Lakeside::start_with(c, |_| None);
    assert!(matches!(l.sign_in(), SignInResult::Failed(_)));
}

#[test]
fn saved_session_is_restored_and_expired_one_is_dropped() {
    let srv = serve(|_| Reply::json(200, "{}"));
    let store = Arc::new(MemoryStore::default());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    store.save(&format!(
        r#"{{"token":"t","name":"Linus","id":"i","saved_at":{now}}}"#
    ));
    let mut c = cfg(&srv.base);
    c.store = store.clone();
    let l = Lakeside::start_with(c, |_| None);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(l.status().account.unwrap().name, "Linus");

    store.save(&format!(
        r#"{{"token":"t","name":"Old","id":"i","saved_at":{}}}"#,
        now - 31 * 86400
    ));
    let mut c = cfg(&srv.base);
    c.store = store.clone();
    let l = Lakeside::start_with(c, |_| None);
    std::thread::sleep(Duration::from_millis(300));
    assert!(l.status().account.is_none());
    assert!(store.load().is_none());
}

// ---- updater ----

fn signed_in(base: &str) -> Lakeside {
    let store = Arc::new(MemoryStore::default());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    store.save(&format!(
        r#"{{"token":"bearer-xyz","name":"P","id":"i","saved_at":{now}}}"#
    ));
    let mut c = cfg(base);
    c.store = store;
    let l = Lakeside::start_with(c, |_| None);
    std::thread::sleep(Duration::from_millis(300));
    l
}

fn listing_json(version: &str, sha: Option<&str>, size: usize) -> String {
    let sha = sha
        .map(|s| format!(r#""sha256":"{s}","#))
        .unwrap_or_default();
    format!(
        r#"{{"version":"{version}","entitled":true,"files":[{{"platform":"{}","name":"setup.bin","size":{size},{sha}"url":"/account/launcher/download/urban-sprawl/{}"}}]}}"#,
        platform_key(),
        platform_key()
    )
}

fn hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

#[test]
fn update_check_states() {
    let body = b"installer bytes".to_vec();
    let sha = hex(&body);
    for (resp, expect) in [
        (
            Reply::json(200, &listing_json("1.0.0-preview.1", Some(&sha), 15)),
            "current",
        ),
        (
            Reply::json(200, &listing_json("1.0.0", Some(&sha), 15)),
            "available",
        ),
        (
            Reply::json(200, &listing_json("1.0.0", None, 15)),
            "nochecksum",
        ),
        (Reply::json(401, "{}"), "signin"),
        (Reply::json(500, "{}"), "unavailable"),
        (
            Reply::json(200, r#"{"version":"2.0.0","entitled":false,"files":[]}"#),
            "notentitled",
        ),
    ] {
        let once = Mutex::new(Some(resp));
        let srv = serve(move |_| once.lock().unwrap().take().unwrap());
        let l = signed_in(&srv.base);
        let got = l.check_update();
        let ok = match expect {
            "current" => got == UpdateCheck::Current,
            "available" => {
                matches!(got, UpdateCheck::Available(ref o) if o.version == "1.0.0" && o.sha256 == sha)
            }
            "nochecksum" => {
                matches!(got, UpdateCheck::Unavailable(ref m) if m.contains("checksum"))
            }
            "signin" => got == UpdateCheck::SignInNeeded,
            "unavailable" => matches!(got, UpdateCheck::Unavailable(_)),
            _ => got == UpdateCheck::NotEntitled,
        };
        assert!(ok, "{expect}: {got:?}");
        let reqs = srv.requests.lock().unwrap();
        assert!(
            reqs[0]
                .to_lowercase()
                .contains("authorization: bearer bearer-xyz")
        );
        if expect == "signin" {
            assert!(l.status().account.is_none(), "401 drops the stale sign-in");
        }
    }
}

#[test]
fn update_signed_out_asks_to_sign_in_without_network() {
    let srv = serve(|_| Reply::json(200, "{}"));
    let l = Lakeside::start_with(cfg(&srv.base), |_| None);
    assert_eq!(l.check_update(), UpdateCheck::SignInNeeded);
    assert!(srv.requests.lock().unwrap().is_empty());
}

#[test]
fn update_disabled_under_launcher() {
    let srv = serve(|_| Reply::json(200, "{}"));
    let l = Lakeside::start_with(cfg(&srv.base), launcher_env(None));
    assert_eq!(l.check_update(), UpdateCheck::Disabled);
    assert!(srv.requests.lock().unwrap().is_empty());
}

fn download_case(
    file: Vec<u8>,
    sha: String,
    size: usize,
) -> (
    Result<std::path::PathBuf, UpdateError>,
    Vec<String>,
    std::path::PathBuf,
) {
    let listing = listing_json("1.0.0", Some(&sha), size);
    let srv = serve(move |req| {
        if req.starts_with("GET /account/api/downloads/") {
            Reply::json(200, &listing)
        } else if req.starts_with("GET /account/launcher/download/") {
            Reply::redirect("/storage/object")
        } else if req.starts_with("GET /storage/object") {
            Reply::bytes(file.clone())
        } else {
            Reply::json(404, "{}")
        }
    });
    let l = signed_in(&srv.base);
    let UpdateCheck::Available(offer) = l.check_update() else {
        panic!("no offer")
    };
    let dir = std::env::temp_dir().join(format!(
        "us-lakeside-test-{}-{}",
        std::process::id(),
        sha.len() + size
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let r = l.download_update(&offer, &dir);
    let reqs = srv.requests.lock().unwrap().clone();
    (r, reqs, dir)
}

#[test]
fn download_verifies_and_drops_token_on_redirect() {
    let body = b"installer bytes".to_vec();
    let (r, reqs, dir) = download_case(body.clone(), hex(&body), body.len());
    let path = r.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), body);
    assert!(reqs[1].to_lowercase().contains("authorization: bearer"));
    assert!(
        !reqs[2].to_lowercase().contains("authorization"),
        "token sent to storage hop"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn download_mismatch_is_thrown_away() {
    let body = b"installer bytes".to_vec();
    let (r, _, dir) = download_case(body.clone(), hex(b"something else"), body.len());
    assert_eq!(r, Err(UpdateError::Mismatch));
    assert!(!dir.join("setup.bin").exists() && !dir.join("setup.bin.part").exists());
    let (r, _, dir2) = download_case(body.clone(), hex(&body), body.len() + 1);
    assert_eq!(r, Err(UpdateError::Mismatch));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

#[test]
fn download_error_statuses_use_cw_messages() {
    for (code, want) in [(401u16, "signin"), (403, "notentitled"), (404, "no build")] {
        let sha = hex(b"x");
        let listing = listing_json("1.0.0", Some(&sha), 1);
        let srv = serve(move |req| {
            if req.starts_with("GET /account/api/downloads/") {
                Reply::json(200, &listing)
            } else {
                Reply::json(code, "{}")
            }
        });
        let l = signed_in(&srv.base);
        let UpdateCheck::Available(o) = l.check_update() else {
            panic!()
        };
        let e = l
            .download_update(&o, &std::env::temp_dir().join("us-lakeside-unused"))
            .unwrap_err();
        match want {
            "signin" => assert_eq!(e, UpdateError::SignInNeeded),
            "notentitled" => assert_eq!(e, UpdateError::NotEntitled),
            _ => assert!(matches!(e, UpdateError::Unavailable(ref m) if m.contains(want))),
        }
    }
}

#[test]
fn version_order() {
    use std::cmp::Ordering::*;
    assert_eq!(compare_versions("1.0.0", "1.0.0-preview.1"), Greater);
    assert_eq!(
        compare_versions("1.0.0-preview.2", "1.0.0-preview.1"),
        Greater
    );
    assert_eq!(
        compare_versions("1.0.0-preview.10", "1.0.0-preview.9"),
        Greater
    );
    assert_eq!(compare_versions("1.0.1", "1.0.0"), Greater);
    assert_eq!(compare_versions("v1.0.0", "1.0.0"), Equal);
}
