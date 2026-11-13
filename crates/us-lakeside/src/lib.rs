//! Lakeside integration for Urban Sprawl: launcher handoff, optional in-game sign-in and the
//! Lakeside updater. No UI. The game always works offline; nothing here is needed to play.
//!
//! Contract: ops-knowledge `lakeside-game-integration-contract-v1`. Behaviour mirrors
//! Corporate Warfare's v1 (`docs/rules/LAUNCHER.md` in that repo), with Urban Sprawl's ids.

#![allow(clippy::result_large_err)]

mod handoff;
mod signin;
mod update;

#[cfg(test)]
mod mock;

pub use handoff::{HandoffCode, redeem};
pub use signin::{KeyringStore, MemoryStore, SignInResult, TokenStore};
pub use update::{UpdateCheck, UpdateError, UpdateOffer, compare_versions, platform_key};

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The account service.
pub const ACCOUNT_BASE: &str = "https://lakesidegames.net";
/// Client id the launcher binds handoff codes to.
pub const CLIENT_ID: &str = "urban-sprawl-desktop";
/// Product id in the Lakeside store and downloads API.
pub const PRODUCT_ID: &str = "urban-sprawl";
/// Where a player gets or downloads the game by hand.
pub const STORE_URL: &str = "https://lakesidegames.net/store/urban-sprawl/";
/// The redeem call is limited to this long, in total.
pub const HANDOFF_TIMEOUT: Duration = Duration::from_secs(5);

/// Longest display name kept.
const NAME_MAX: usize = 40;

/// Who is signed in. In memory only.
#[derive(Clone, PartialEq, Eq)]
pub struct Account {
    /// Lakeside account id (empty when sign-in did not report one).
    pub id: String,
    /// Cleaned display name, never empty ("your Lakeside account" when blank).
    pub name: String,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

pub(crate) const FALLBACK_NAME: &str = "your Lakeside account";

/// Trim, drop control characters, cut at 40 characters.
pub(crate) fn clean_name(raw: &str) -> String {
    let kept: String = raw.chars().filter(|c| !c.is_control()).collect();
    kept.trim()
        .chars()
        .take(NAME_MAX)
        .collect::<String>()
        .trim()
        .to_string()
}

/// What the app shows. Cheap to copy; call every frame if you like.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// True when the Lakeside launcher started this copy: hide all updater UI.
    pub launcher: bool,
    /// The launcher's version, empty when it did not say or there is no launcher.
    pub launcher_version: String,
    /// Set when a launcher handoff or an in-game sign-in succeeded.
    pub account: Option<Account>,
    /// True when the in-game updater may be offered (no launcher).
    pub updater_enabled: bool,
    /// True while the launcher handoff is still being redeemed in the background.
    pub handoff_pending: bool,
}

/// Settings. Tests point `base` at a local server.
#[derive(Clone)]
pub struct Config {
    pub base: String,
    pub current_version: String,
    pub handoff_timeout: Duration,
    pub ports: std::ops::RangeInclusive<u16>,
    pub sign_in_wait: Duration,
    pub open_url: fn(&str) -> bool,
    pub store: Arc<dyn TokenStore>,
}

impl Config {
    /// Real service, OS secure storage for the sign-in token.
    pub fn production(current_version: &str) -> Self {
        Self {
            base: ACCOUNT_BASE.into(),
            current_version: current_version.into(),
            handoff_timeout: HANDOFF_TIMEOUT,
            ports: signin::PORTS,
            sign_in_wait: Duration::from_secs(300),
            open_url: signin::open_in_browser,
            store: Arc::new(KeyringStore::new()),
        }
    }
}

#[derive(Default)]
struct State {
    launcher: bool,
    launcher_version: String,
    account: Option<Account>,
    handoff_pending: bool,
    token: Option<signin::Saved>,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}

/// The handle the app keeps. Clone it freely.
#[derive(Clone)]
pub struct Lakeside {
    state: Arc<Mutex<State>>,
    cfg: Arc<Config>,
}

impl std::fmt::Debug for Lakeside {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lakeside")
            .field("status", &self.status())
            .finish()
    }
}

impl Lakeside {
    /// Start at launch. Reads the launcher environment, redeems a handoff code once on a
    /// background thread, and restores a saved in-game sign-in. Never blocks the caller.
    pub fn start() -> Self {
        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");
        Self::start_with(Config::production(version), |k| std::env::var(k).ok())
    }

    /// As [`start`](Self::start) with explicit settings and environment (tests).
    pub fn start_with(cfg: Config, env: impl Fn(&str) -> Option<String>) -> Self {
        let managed = env("LAKESIDE_LAUNCHER").as_deref() == Some("1");
        let launcher_version = if managed {
            clean_name(&env("LAKESIDE_LAUNCHER_VERSION").unwrap_or_default())
                .chars()
                .take(32)
                .collect()
        } else {
            String::new()
        };
        // The code is only read when the launcher started this copy.
        let code = if managed {
            env("LAKESIDE_HANDOFF_CODE")
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .map(HandoffCode::new)
        } else {
            None
        };
        let state = State {
            launcher: managed,
            launcher_version,
            handoff_pending: code.is_some(),
            ..State::default()
        };
        let this = Self {
            state: Arc::new(Mutex::new(state)),
            cfg: Arc::new(cfg),
        };

        let worker = this.clone();
        std::thread::Builder::new()
            .name("lakeside-start".into())
            .spawn(move || {
                if let Some(code) = code {
                    let account =
                        handoff::redeem(&worker.cfg.base, code, worker.cfg.handoff_timeout);
                    let mut s = worker.lock();
                    s.handoff_pending = false;
                    if account.is_some() {
                        s.account = account;
                    }
                }
                if !worker.lock().launcher || worker.lock().account.is_none() {
                    worker.restore();
                }
            })
            .ok();
        this
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Current status.
    pub fn status(&self) -> Status {
        let s = self.lock();
        Status {
            launcher: s.launcher,
            launcher_version: s.launcher_version.clone(),
            account: s.account.clone(),
            updater_enabled: !s.launcher,
            handoff_pending: s.handoff_pending,
        }
    }

    fn restore(&self) {
        if let Some(saved) = signin::load(self.cfg.store.as_ref()) {
            let mut s = self.lock();
            if s.account.is_none() {
                s.account = Some(Account {
                    id: saved.id.clone(),
                    name: saved.name.clone(),
                });
            }
            s.token = Some(saved);
        }
    }

    /// Open the browser and wait for the player to finish. Blocking: call it off the main thread.
    pub fn sign_in(&self) -> SignInResult {
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.lock().cancel = Some(cancel.clone());
        let result = signin::run(&self.cfg, &cancel);
        let mut s = self.lock();
        s.cancel = None;
        match result {
            Ok(saved) => {
                let name = saved.name.clone();
                s.account = Some(Account {
                    id: saved.id.clone(),
                    name: name.clone(),
                });
                s.token = Some(saved.clone());
                drop(s);
                signin::save(self.cfg.store.as_ref(), &saved);
                SignInResult::Done { name }
            }
            Err(other) => other,
        }
    }

    /// Stop a sign-in that is waiting on the browser.
    pub fn cancel_sign_in(&self) {
        if let Some(c) = &self.lock().cancel {
            c.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Forget the in-game sign-in here: removes it from secure storage and memory. A launcher
    /// handoff account is also forgotten for this session.
    pub fn sign_out(&self) {
        {
            let mut s = self.lock();
            s.token = None;
            s.account = None;
        }
        self.cfg.store.delete();
    }

    /// Check for a newer build through Lakeside. Blocking. `Disabled` when the launcher owns updates.
    pub fn check_update(&self) -> UpdateCheck {
        if self.lock().launcher {
            return UpdateCheck::Disabled;
        }
        let token = self.fresh_token();
        let result = update::check(&self.cfg, token.as_deref());
        if matches!(result, UpdateCheck::SignInNeeded) {
            self.sign_out();
        }
        result
    }

    /// Download and verify the offered build into `dir`. Blocking. Returns the verified file.
    pub fn download_update(
        &self,
        offer: &UpdateOffer,
        dir: &std::path::Path,
    ) -> Result<std::path::PathBuf, UpdateError> {
        if self.lock().launcher {
            return Err(UpdateError::Unavailable(
                "The Lakeside launcher keeps this copy up to date.".into(),
            ));
        }
        let token = self.fresh_token().ok_or(UpdateError::SignInNeeded)?;
        let result = update::download(&self.cfg, &token, offer, dir);
        if matches!(result, Err(UpdateError::SignInNeeded)) {
            self.sign_out();
        }
        result
    }

    fn fresh_token(&self) -> Option<String> {
        let mut s = self.lock();
        let expired = s.token.as_ref().is_some_and(signin::expired);
        if expired {
            s.token = None;
            s.account = None;
            drop(s);
            self.cfg.store.delete();
            return None;
        }
        s.token.as_ref().map(|t| t.token.clone())
    }
}

#[cfg(test)]
mod tests;
