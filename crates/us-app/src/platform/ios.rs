//! iOS platform code. There is no dialog to open from here, so a start failure goes to the log.

pub fn tell_user(reason: &str) {
    eprintln!("Urban Sprawl could not start: {reason}");
}
