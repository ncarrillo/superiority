#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main() {
    if std::env::args().any(|argument| argument == "--protocol-viewer") {
        superiority_app::app::run_protocol_viewer();
    } else {
        superiority_app::app::run();
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    eprintln!("The desktop client currently supports macOS and Windows only.");
}
