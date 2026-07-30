// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Initialize tracing subscriber to pick up RUST_LOG env var
    tracing_subscriber::fmt::init();
    gullbur_desktop_lib::run()
}
