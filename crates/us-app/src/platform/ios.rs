//! iOS platform code: the entry point the Xcode project calls. The safe area (notch, rounded
//! corners, home indicator) comes from egui-winit, which hands it to the interface.

/// Called from the app's `main` (see `packaging/ios`). Never returns: UIKit runs the event loop.
#[unsafe(no_mangle)]
pub extern "C" fn urban_sprawl_ios_main() {
    crate::run();
}

/// There is no dialog to open from here, so a start failure goes to the log.
pub fn tell_user(reason: &str) {
    eprintln!("Urban Sprawl could not start: {reason}");
}
