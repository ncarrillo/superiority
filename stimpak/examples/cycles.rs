//! opens sessions, connects them, and closes them mid-flight — the shape a
//! caller-owned reconnect actually takes. reports how long close takes and
//! whether the process settles back to its baseline thread count.
use std::{ffi::CString, time::{Duration, Instant}};

fn threads() -> usize {
    let output = std::process::Command::new("ps")
        .args(["-M", &std::process::id().to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout).lines().count().saturating_sub(1)
}

fn main() {
    let path = std::env::temp_dir().join("stimpak-cycles.bin");
    let path = CString::new(path.to_string_lossy().as_ref()).unwrap();
    let cycles: usize = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(8);
    let dwell = Duration::from_millis(
        std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(400),
    );

    println!("baseline threads: {}", threads());
    let mut slowest = Duration::ZERO;

    for cycle in 1..=cycles {
        let client = unsafe { stimpak::stimpak_client_open(path.as_ptr(), std::ptr::null()) };
        assert!(!client.is_null());
        // connect for real, then tear it down while it is still working
        unsafe { stimpak::stimpak_client_connect(client, false) };
        std::thread::sleep(dwell);
        let started = Instant::now();
        unsafe { stimpak::stimpak_client_close(client) };
        let took = started.elapsed();
        slowest = slowest.max(took);
        println!("cycle {cycle:2}: close took {:>6.1?}, threads now {}", took, threads());
    }

    std::thread::sleep(Duration::from_secs(2));
    println!("slowest close: {slowest:.1?}");
    println!("settled threads: {}", threads());
}
