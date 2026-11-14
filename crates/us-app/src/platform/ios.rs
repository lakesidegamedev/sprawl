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

#[repr(C)]
#[derive(Clone, Copy)]
struct EdgeInsets {
    top: f64,
    left: f64,
    bottom: f64,
    right: f64,
}

// SAFETY: same layout and encoding as UIKit's UIEdgeInsets (four 64 bit CGFloat).
unsafe impl objc2::Encode for EdgeInsets {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct(
        "UIEdgeInsets",
        &[
            objc2::Encoding::Double,
            objc2::Encoding::Double,
            objc2::Encoding::Double,
            objc2::Encoding::Double,
        ],
    );
}

/// Room the notch, rounded corners and home indicator take, in points: top, left, bottom, right.
pub fn safe_insets(window: &winit::window::Window) -> [f32; 4] {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return [0.0; 4];
    };
    let RawWindowHandle::UiKit(h) = handle.as_raw() else {
        return [0.0; 4];
    };
    // SAFETY: `ui_view` is the window's live UIView and this runs on the main thread.
    let i: EdgeInsets = unsafe {
        let view = &*(h.ui_view.as_ptr() as *const objc2::runtime::AnyObject);
        objc2::msg_send![view, safeAreaInsets]
    };
    [i.top as f32, i.left as f32, i.bottom as f32, i.right as f32]
}
