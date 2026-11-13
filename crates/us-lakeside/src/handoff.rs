//! The launcher handoff: redeem the single use code once, v1 rules.

use crate::{Account, CLIENT_ID, FALLBACK_NAME, clean_name};
use serde::Deserialize;
use std::io::Read;
use std::time::Duration;

const MAX_BODY: u64 = 16 * 1024;

/// The single use handoff code. Its Debug output never shows the value, and nothing stores it.
pub struct HandoffCode(String);

impl HandoffCode {
    pub(crate) fn new(code: String) -> Self {
        Self(code)
    }
}

impl std::fmt::Debug for HandoffCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HandoffCode([redacted])")
    }
}

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "accountId")]
    account_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

/// One POST, total limit `timeout`, no redirects, never retried. Consumes the code. Any failure
/// is `None` ("not signed in"). Nothing is logged.
pub fn redeem(base: &str, code: HandoffCode, timeout: Duration) -> Option<Account> {
    let agent = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(0)
        .build();
    let body = serde_json::json!({ "code": code.0, "clientId": CLIENT_ID });
    let url = format!("{base}/account/launcher/handoff/redeem");
    let response = agent
        .post(&url)
        .set("Accept", "application/json")
        .send_json(body)
        .ok()?;
    if response.status() != 200 {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_BODY {
        return None;
    }
    parse(&bytes)
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Account> {
    let reply: Reply = serde_json::from_slice(bytes).ok()?;
    let id = reply.account_id?.trim().to_string();
    if id.is_empty() || id.chars().count() > 128 {
        return None;
    }
    let name = clean_name(&reply.display_name.unwrap_or_default());
    Some(Account {
        id,
        name: if name.is_empty() {
            FALLBACK_NAME.into()
        } else {
            name
        },
    })
}
