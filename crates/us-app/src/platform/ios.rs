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
    // SAFETY: `ui_view` is the window's live UIView and this runs on the main thread. Ask the view
    // and the UIWindow it sits in: either may be the one that has the numbers.
    let (a, b): (EdgeInsets, EdgeInsets) = unsafe {
        let view = &*(h.ui_view.as_ptr() as *const objc2::runtime::AnyObject);
        let a: EdgeInsets = objc2::msg_send![view, safeAreaInsets];
        let win: *const objc2::runtime::AnyObject = objc2::msg_send![view, window];
        let b = if win.is_null() {
            a
        } else {
            objc2::msg_send![&*win, safeAreaInsets]
        };
        (a, b)
    };
    let mut v = [
        a.top.max(b.top) as f32,
        a.left.max(b.left) as f32,
        a.bottom.max(b.bottom) as f32,
        a.right.max(b.right) as f32,
    ];
    let phone = {
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        size.width.min(size.height) as f32 / scale < 600.0
    };
    if phone && v.iter().all(|x| *x <= 0.0) {
        // UIKit gave nothing: keep clear of the Dynamic Island, rounded corners and home bar
        // the way a current iPhone needs, so controls are never under them.
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let (w, h) = (size.width as f32 / scale, size.height as f32 / scale);
        v = if w < h {
            [54.0, 0.0, 22.0, 0.0]
        } else {
            [0.0, 54.0, 20.0, 54.0]
        };
    }
    v
}
