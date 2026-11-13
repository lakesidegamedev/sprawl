//! Urban Sprawl desktop binary: all the work is in the `us_app` library.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

fn main() {
    us_app::run();
}
