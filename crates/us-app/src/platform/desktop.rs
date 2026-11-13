//! Desktop platform code: how a start failure is shown to the player.

/// Say why the game cannot start, in a window the player sees, not only in the log. Best effort:
/// a native message box on Windows and macOS, zenity, kdialog or xmessage on Linux.
pub fn tell_user(reason: &str) {
    let text = format!(
        "Urban Sprawl could not start.\n\n{reason}\n\nMore detail is in {}",
        crate::log_path().display()
    );
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let (body, title) = (wide(&text), wide("Urban Sprawl"));
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                body.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display alert \"Urban Sprawl could not start\" message {:?}",
            text
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::Command;
        let ok = |c: &mut Command| c.status().map(|s| s.success()).unwrap_or(false);
        let shown = ok(Command::new("zenity")
            .args(["--error", "--no-wrap", "--title=Urban Sprawl"])
            .arg(format!("--text={text}")))
            || ok(Command::new("kdialog")
                .args(["--title", "Urban Sprawl", "--error"])
                .arg(&text));
        if !shown {
            let _ = Command::new("xmessage").arg(&text).status();
        }
    }
}
