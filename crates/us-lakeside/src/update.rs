//! The Lakeside updater: signed-in downloads API, size and SHA-256 verified, no checksum means refuse.

use crate::{Config, PRODUCT_ID};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const OFFLINE: &str =
    "Could not reach Lakeside. Your game works as before; try again later, or use Get it.";
const MAX_HOPS: usize = 4;

/// The download key for this computer.
pub fn platform_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "macos-universal"
    } else {
        "linux-x86_64"
    }
}

/// A newer build the signed-in player may download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOffer {
    pub version: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    /// Path on the account service, never an absolute URL.
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateCheck {
    /// The launcher owns updates.
    Disabled,
    Current,
    Available(UpdateOffer),
    /// Not signed in (or the session was rejected). Offer Sign in, or open `STORE_URL`.
    SignInNeeded,
    NotEntitled,
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    SignInNeeded,
    NotEntitled,
    Unavailable(String),
    /// The file did not match its published size or checksum and was thrown away.
    Mismatch,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignInNeeded => f.write_str("Sign in to Lakeside to download updates."),
            Self::NotEntitled => f.write_str("This Lakeside account does not own Urban Sprawl."),
            Self::Unavailable(s) => f.write_str(s),
            Self::Mismatch => f.write_str("The download did not match the checksum Lakeside published for it, so it was thrown away and nothing was installed."),
        }
    }
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    platform: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    size: u64,
    sha256: Option<String>,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct Listing {
    version: Option<String>,
    #[serde(default)]
    entitled: bool,
    #[serde(default)]
    files: Vec<File>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .redirects(0)
        .build()
}

fn get(
    agent: &ureq::Agent,
    url: &str,
    token: Option<&str>,
    ver: &str,
) -> Result<ureq::Response, ureq::Error> {
    let mut r = agent
        .get(url)
        .set("Accept", "application/json")
        .set("User-Agent", &format!("UrbanSprawl/{ver}"));
    if let Some(t) = token {
        r = r.set("Authorization", &format!("Bearer {t}"));
    }
    r.call()
}

/// `a` against `b`: MAJOR.MINOR.PATCH, then a release beats any prerelease of it, prereleases by dotted parts.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(v: &str) -> ([u64; 3], Option<String>) {
        let v = v.trim().trim_start_matches('v');
        let (core, pre) = match v.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (v, None),
        };
        let mut n = [0u64; 3];
        for (i, p) in core.split('.').take(3).enumerate() {
            n[i] = p.parse().unwrap_or(0);
        }
        (n, pre)
    }
    let (an, ap) = split(a);
    let (bn, bp) = split(b);
    an.cmp(&bn).then_with(|| match (ap, bp) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => {
            let (xs, ys): (Vec<&str>, Vec<&str>) = (x.split('.').collect(), y.split('.').collect());
            for (p, q) in xs.iter().zip(ys.iter()) {
                let o = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    _ => p.cmp(q),
                };
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            xs.len().cmp(&ys.len())
        }
    })
}

fn listing(cfg: &Config, token: &str) -> Result<Listing, UpdateError> {
    let url = format!("{}/account/api/downloads/{PRODUCT_ID}", cfg.base);
    match get(&agent(), &url, Some(token), &cfg.current_version) {
        Ok(r) if r.status() == 200 => {
            let mut bytes = Vec::new();
            r.into_reader()
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|_| UpdateError::Unavailable(OFFLINE.into()))?;
            serde_json::from_slice(&bytes).map_err(|_| {
                UpdateError::Unavailable(
                    "Lakeside sent back something unexpected. Try again later.".into(),
                )
            })
        }
        Ok(r) => Err(UpdateError::Unavailable(format!(
            "Lakeside could not list the download (error {}). Try again later, or use Get it.",
            r.status()
        ))),
        Err(ureq::Error::Status(401, _)) => Err(UpdateError::SignInNeeded),
        Err(ureq::Error::Status(c, _)) => Err(UpdateError::Unavailable(format!(
            "Lakeside could not list the download (error {c}). Try again later, or use Get it."
        ))),
        Err(_) => Err(UpdateError::Unavailable(OFFLINE.into())),
    }
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn check(cfg: &Config, token: Option<&str>) -> UpdateCheck {
    let Some(token) = token else {
        return UpdateCheck::SignInNeeded;
    };
    let list = match listing(cfg, token) {
        Ok(l) => l,
        Err(UpdateError::SignInNeeded) => return UpdateCheck::SignInNeeded,
        Err(UpdateError::Unavailable(s)) => return UpdateCheck::Unavailable(s),
        Err(e) => return UpdateCheck::Unavailable(e.to_string()),
    };
    if !list.entitled {
        return UpdateCheck::NotEntitled;
    }
    let Some(version) = list.version.clone() else {
        return UpdateCheck::Current;
    };
    if compare_versions(&version, &cfg.current_version) != std::cmp::Ordering::Greater {
        return UpdateCheck::Current;
    }
    let Some(file) = list.files.iter().find(|f| f.platform == platform_key()) else {
        return UpdateCheck::Unavailable(
            "There is no build for this computer in your Lakeside library yet.".into(),
        );
    };
    let sha = file
        .sha256
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if !is_sha256(&sha) {
        return UpdateCheck::Unavailable(
            "Lakeside did not publish a checksum for this download, so nothing was installed."
                .into(),
        );
    }
    let name_ok = !file.name.is_empty()
        && file.name.len() <= 120
        && file
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    if !name_ok || !file.url.starts_with('/') || file.url.starts_with("//") {
        return UpdateCheck::Unavailable(
            "Lakeside sent a download this game does not trust, so nothing was installed.".into(),
        );
    }
    UpdateCheck::Available(UpdateOffer {
        version,
        file_name: file.name.clone(),
        size: file.size,
        sha256: sha,
        path: file.url.clone(),
    })
}

pub(crate) fn download(
    cfg: &Config,
    token: &str,
    offer: &UpdateOffer,
    dir: &Path,
) -> Result<PathBuf, UpdateError> {
    let unavailable = |s: &str| UpdateError::Unavailable(s.to_string());
    if !is_sha256(&offer.sha256) {
        return Err(unavailable(
            "Lakeside did not publish a checksum for this download, so nothing was installed.",
        ));
    }
    if !offer.path.starts_with('/')
        || offer.path.starts_with("//")
        || offer.file_name.contains(['/', '\\'])
        || offer.file_name.starts_with('.')
    {
        return Err(unavailable(
            "Lakeside sent a download this game does not trust, so nothing was installed.",
        ));
    }
    let home = cfg.base.clone();
    let ag = agent();
    let mut url = format!("{home}{}", offer.path);
    let mut bearer = Some(token);
    for _ in 0..=MAX_HOPS {
        match get(&ag, &url, bearer, &cfg.current_version) {
            Ok(r) if (300..400).contains(&r.status()) => {
                let loc = r
                    .header("Location")
                    .ok_or_else(|| unavailable(OFFLINE))?
                    .to_string();
                let next = if loc.starts_with('/') {
                    format!("{home}{loc}")
                } else {
                    loc
                };
                // Only the storage link over https, or Lakeside itself; and the sign-in never goes with it.
                if !next.starts_with("https://") && !next.starts_with(&home) {
                    return Err(unavailable(
                        "Lakeside sent a download this game does not trust, so nothing was installed.",
                    ));
                }
                url = next;
                bearer = None;
            }
            Ok(r) if r.status() == 200 => return stream(r, offer, dir),
            Ok(r) => {
                return Err(unavailable(&format!(
                    "The download failed (error {}). Try again later, or use Get it.",
                    r.status()
                )));
            }
            Err(ureq::Error::Status(401, _)) => return Err(UpdateError::SignInNeeded),
            Err(ureq::Error::Status(403, _)) => return Err(UpdateError::NotEntitled),
            Err(ureq::Error::Status(404, _)) => {
                return Err(unavailable(
                    "There is no build for this computer in your Lakeside library yet.",
                ));
            }
            Err(ureq::Error::Status(c, _)) => {
                return Err(unavailable(&format!(
                    "The download failed (error {c}). Try again later, or use Get it."
                )));
            }
            Err(_) => return Err(unavailable(OFFLINE)),
        }
    }
    Err(unavailable(OFFLINE))
}

fn stream(resp: ureq::Response, offer: &UpdateOffer, dir: &Path) -> Result<PathBuf, UpdateError> {
    let fail = || {
        UpdateError::Unavailable("The download did not finish. Try again, or use Get it.".into())
    };
    std::fs::create_dir_all(dir).map_err(|_| fail())?;
    let target = dir.join(&offer.file_name);
    let partial = dir.join(format!("{}.part", offer.file_name));
    let mut out = std::fs::File::create(&partial).map_err(|_| fail())?;
    let mut hash = Sha256::new();
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(_) => {
                let _ = std::fs::remove_file(&partial);
                return Err(fail());
            }
        };
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        total += n as u64;
        if out.write_all(&buf[..n]).is_err() || (offer.size > 0 && total > offer.size) {
            let _ = std::fs::remove_file(&partial);
            return Err(UpdateError::Mismatch);
        }
    }
    drop(out);
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if (offer.size > 0 && total != offer.size) || digest != offer.sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(UpdateError::Mismatch);
    }
    let _ = std::fs::remove_file(&target);
    std::fs::rename(&partial, &target).map_err(|_| fail())?;
    Ok(target)
}
