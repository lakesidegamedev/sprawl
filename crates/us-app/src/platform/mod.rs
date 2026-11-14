//! Code that differs by platform, kept out of the shared app so UI work does not collide with it.

#[cfg(not(target_os = "ios"))]
mod desktop;
#[cfg(target_os = "ios")]
mod ios;

#[cfg(not(target_os = "ios"))]
pub use desktop::{safe_insets, tell_user};
#[cfg(target_os = "ios")]
pub use ios::{safe_insets, tell_user};
