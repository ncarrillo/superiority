mod packet;
mod platform;
mod session;
mod tui;

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use std::os::unix::fs::OpenOptionsExt;

use sc2_core::native::Protocol;

use crate::session::{Sweep, SweepUpdate};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("{0}")]
    Capture(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Pcap(#[from] pcap_file::PcapError),
    #[error(transparent)]
    Protocol(#[from] sc2_core::Error),
    #[error("{0}")]
    Bootstrap(String),
    #[error("{0}")]
    Platform(String),
    #[error("{0}")]
    UnknownPacket(Box<session::UnknownPacket>),
}

type Result<T> = std::result::Result<T, Error>;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    match arguments.first().and_then(|value| value.to_str()) {
        None | Some("live") => run_live(&arguments),
        Some("replay") => run_replay(&arguments),
        Some("help" | "--help" | "-h") => {
            print_usage();
            Ok(())
        }
        Some(command) => Err(Error::Bootstrap(format!(
            "unknown scanner-sweep command {command}"
        ))),
    }
}

fn run_replay(arguments: &[std::ffi::OsString]) -> Result<()> {
    let mut pcap = None;
    let mut game_utilities = None;
    let mut protocol_executable = None;
    let mut headless = false;
    let mut index = 1;
    while index < arguments.len() {
        let name = arguments[index].to_string_lossy();
        if name == "--headless" {
            headless = true;
            index += 1;
            continue;
        }
        let destination = match name.as_ref() {
            "--pcap" => &mut pcap,
            "--game-utilities" => &mut game_utilities,
            "--protocol-executable" => &mut protocol_executable,
            _ => {
                return Err(Error::Bootstrap(format!(
                    "unknown scanner-sweep argument {name}"
                )));
            }
        };
        index += 1;
        let value = arguments.get(index).ok_or_else(|| {
            Error::Bootstrap(format!("scanner-sweep argument {name} needs a path"))
        })?;
        *destination = Some(PathBuf::from(value));
        index += 1;
    }
    let pcap = pcap.ok_or_else(|| Error::Bootstrap("replay requires --pcap".to_owned()))?;
    let game_utilities = game_utilities
        .ok_or_else(|| Error::Bootstrap("replay requires --game-utilities".to_owned()))?;
    if headless {
        let protocol = load_protocol(protocol_executable)?;
        replay(protocol, &pcap, &game_utilities, print_updates)
    } else {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = load_protocol(protocol_executable).and_then(|protocol| {
                replay(protocol, &pcap, &game_utilities, |updates| {
                    for update in updates {
                        if sender.send(tui::Event::Update(update)).is_err() {
                            return;
                        }
                    }
                })
            });
            let _ = sender.send(match result {
                Ok(()) => tui::Event::Complete,
                Err(error) => tui::Event::Failed(error),
            });
        });
        tui::run(&receiver)
    }
}

#[derive(Debug)]
enum SourceEvent {
    Packet(packet::TcpPacket),
    Bootstrap(Vec<u8>),
    Reset,
    Status(String),
    Failed(Error),
}

#[cfg(target_os = "macos")]
#[expect(
    clippy::too_many_lines,
    reason = "live capture wires three long-lived workers"
)]
fn run_live(arguments: &[std::ffi::OsString]) -> Result<()> {
    if !cfg!(target_arch = "x86_64") {
        return Err(Error::Platform(
            "live mode must use the x86_64 scanner-sweep binary because SC2 runs under Rosetta; run `cargo run -p scanner-sweep --target x86_64-apple-darwin` as your macOS user"
                .to_owned(),
        ));
    }
    if unsafe { libc::geteuid() } == 0 {
        return Err(Error::Platform(
            "do not run scanner-sweep with sudo; run the x86_64 binary as your macOS user and scanner-sweep will elevate only tcpdump"
                .to_owned(),
        ));
    }

    let mut output = None;
    let mut protocol_executable = None;
    let mut launch = true;
    let mut index = usize::from(arguments.first().and_then(|value| value.to_str()) == Some("live"));
    while index < arguments.len() {
        let name = arguments[index].to_string_lossy();
        if name == "--no-launch" {
            launch = false;
            index += 1;
            continue;
        }
        let destination = match name.as_ref() {
            "--output" => &mut output,
            "--protocol-executable" => &mut protocol_executable,
            _ => {
                return Err(Error::Bootstrap(format!(
                    "unknown scanner-sweep argument {name}"
                )));
            }
        };
        index += 1;
        let value = arguments.get(index).ok_or_else(|| {
            Error::Bootstrap(format!("scanner-sweep argument {name} needs a path"))
        })?;
        *destination = Some(PathBuf::from(value));
        index += 1;
    }
    wait_for_clean_launch()?;

    let protocol = load_live_protocol(protocol_executable)?;
    packet::live::authorize()?;
    let capture_path = output.unwrap_or_else(default_capture_path);
    if let Some(parent) = capture_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bootstrap_path = capture_path.with_extension("game-utilities.jsonl");
    let stop = Arc::new(AtomicBool::new(false));
    let (source_sender, source_receiver) = mpsc::channel();
    let (packet_sender, packet_receiver) = mpsc::channel();
    let (ready_sender, ready_receiver) = mpsc::channel();

    let capture_stop = Arc::clone(&stop);
    let capture_events = source_sender.clone();
    let capture_output = capture_path.clone();
    thread::spawn(move || {
        if let Err(error) = packet::live::run(
            &capture_output,
            &capture_stop,
            &packet_sender,
            &ready_sender,
        ) {
            let _ = capture_events.send(SourceEvent::Failed(error));
        }
    });
    match ready_receiver.recv().map_err(|_| {
        Error::Capture("packet-capture worker exited before initialization".to_owned())
    })? {
        Ok(()) => {}
        Err(message) => return Err(Error::Capture(message)),
    }
    source_sender
        .send(SourceEvent::Status(format!(
            "encrypted traffic streaming to {}",
            capture_path.display()
        )))
        .map_err(|_| Error::Capture("capture event stream closed".to_owned()))?;

    let packet_events = source_sender.clone();
    thread::spawn(move || {
        while let Ok(packet) = packet_receiver.recv() {
            if packet_events.send(SourceEvent::Packet(packet)).is_err() {
                break;
            }
        }
    });

    let hook_stop = Arc::clone(&stop);
    let hook_events = source_sender.clone();
    thread::spawn(move || {
        let result = (|| -> Result<()> {
            hook_events
                .send(SourceEvent::Status(
                    "waiting for SC2 and the GameUtilities response".to_owned(),
                ))
                .map_err(|_| Error::Platform("capture event stream closed".to_owned()))?;
            let lldb_output = bootstrap_path.with_extension("lldb.jsonl");
            let mut pid = platform::wait_for_sc2(&hook_stop)?;
            let mut attempt = 1;
            let response = loop {
                hook_events
                    .send(SourceEvent::Status(format!(
                        "starting the targeted LLDB GameUtilities hook for SC2 PID {pid}"
                    )))
                    .map_err(|_| Error::Platform("capture event stream closed".to_owned()))?;
                match platform::capture_game_utilities(pid, &lldb_output) {
                    Ok(response) => break response,
                    Err(error) if attempt < 4 && is_retryable_lldb_startup_error(&error) => {
                        attempt += 1;
                        hook_events
                            .send(SourceEvent::Status(format!(
                                "LLDB lost the early SC2 session; relaunching SC2 ({attempt}/4)"
                            )))
                            .map_err(|_| {
                                Error::Platform("capture event stream closed".to_owned())
                            })?;
                        close_sc2(pid)?;
                        hook_events.send(SourceEvent::Reset).map_err(|_| {
                            Error::Platform("capture event stream closed".to_owned())
                        })?;
                        platform::launch_sc2()?;
                        pid = platform::wait_for_sc2(&hook_stop)?;
                    }
                    Err(error) => return Err(error),
                }
            };
            write_bootstrap(&bootstrap_path, pid, &response)?;
            hook_events
                .send(SourceEvent::Bootstrap(response))
                .map_err(|_| Error::Platform("capture event stream closed".to_owned()))?;
            Ok(())
        })();
        if let Err(error) = result
            && !hook_stop.load(Ordering::Relaxed)
        {
            let _ = hook_events.send(SourceEvent::Failed(error));
        }
    });

    if launch {
        platform::launch_sc2()?;
    }

    let (tui_sender, tui_receiver) = mpsc::channel();
    let sweep_stop = Arc::clone(&stop);
    thread::spawn(move || {
        let mut sweep = Sweep::new(protocol);
        while let Ok(event) = source_receiver.recv() {
            let result = match event {
                SourceEvent::Packet(packet) => sweep.ingest(&packet),
                SourceEvent::Bootstrap(response) => sweep.set_bootstrap(&response),
                SourceEvent::Reset => {
                    sweep.reset();
                    Ok(Vec::new())
                }
                SourceEvent::Status(status) => Ok(vec![SweepUpdate::Status(status)]),
                SourceEvent::Failed(error) => Err(error),
            };
            match result {
                Ok(updates) => {
                    for update in updates {
                        if tui_sender.send(tui::Event::Update(update)).is_err() {
                            sweep_stop.store(true, Ordering::Relaxed);
                            return;
                        }
                    }
                }
                Err(error) => {
                    sweep_stop.store(true, Ordering::Relaxed);
                    let _ = tui_sender.send(tui::Event::Failed(error));
                    return;
                }
            }
        }
    });

    let result = tui::run(&tui_receiver);
    stop.store(true, Ordering::Relaxed);
    result
}

#[cfg(not(target_os = "macos"))]
fn run_live(_arguments: &[std::ffi::OsString]) -> Result<()> {
    Err(Error::Platform(
        "live scanner-sweep capture is currently macOS-only; replay mode is portable".to_owned(),
    ))
}

#[cfg(target_os = "macos")]
fn wait_for_clean_launch() -> Result<()> {
    let Some(pid) = platform::find_sc2()? else {
        return Ok(());
    };
    print!("SC2 is running. Ask it to quit and continue? [y/N] ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    if !matches!(line.trim(), "y" | "Y" | "yes" | "YES" | "Yes") {
        return Err(Error::Platform("capture cancelled".to_owned()));
    }
    close_sc2(pid)
}

#[cfg(target_os = "macos")]
fn close_sc2(pid: i32) -> Result<()> {
    let _ = platform::request_sc2_quit(pid);
    if wait_for_sc2_exit(50)? {
        return Ok(());
    }
    platform::terminate_sc2(pid, false)?;
    if wait_for_sc2_exit(50)? {
        return Ok(());
    }
    platform::terminate_sc2(pid, true)?;
    if wait_for_sc2_exit(30)? {
        return Ok(());
    }
    Err(Error::Platform("SC2 could not be closed".to_owned()))
}

fn is_retryable_lldb_startup_error(error: &Error) -> bool {
    matches!(
        error,
        Error::Platform(message)
            if message.contains("EXC_BAD_ACCESS (code=1, address=0x0)")
                || message.contains("attach failed: lost connection")
    )
}

#[cfg(target_os = "macos")]
fn wait_for_sc2_exit(ticks: usize) -> Result<bool> {
    for _ in 0..ticks {
        if platform::find_sc2()?.is_none() {
            return Ok(true);
        }
        thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(platform::find_sc2()?.is_none())
}

fn default_capture_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("captures")
        .join(format!("scanner-sweep-{timestamp}.pcapng"))
}

#[cfg(target_os = "macos")]
fn write_bootstrap(path: &Path, pid: i32, response: &[u8]) -> Result<()> {
    let document = serde_json::json!({
        "type": "game_utilities_client_response",
        "pid": pid,
        "length": response.len(),
        "hex": hex::encode(response),
    });
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer(&mut file, &document)
        .map_err(|error| Error::Bootstrap(format!("could not encode bootstrap log: {error}")))?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn load_protocol(executable: Option<PathBuf>) -> Result<Protocol> {
    executable
        .map_or_else(Protocol::current, Protocol::from_executable)
        .map_err(Into::into)
}

#[cfg(target_os = "macos")]
fn load_live_protocol(executable: Option<PathBuf>) -> Result<Protocol> {
    let executable = executable.map_or_else(platform::installed_sc2_executable, Ok)?;
    Protocol::from_executable(executable).map_err(Into::into)
}

fn replay(
    protocol: Protocol,
    pcap: &std::path::Path,
    game_utilities: &std::path::Path,
    mut update: impl FnMut(Vec<SweepUpdate>),
) -> Result<()> {
    let bootstrap = session::load_bootstrap(game_utilities)?;
    let packets = packet::read_capture(pcap)?;
    let mut sweep = Sweep::new(protocol);
    update(sweep.set_bootstrap(&bootstrap)?);
    for packet in packets {
        update(sweep.ingest(&packet)?);
    }
    update(sweep.finish()?);
    Ok(())
}

fn print_updates(updates: Vec<SweepUpdate>) {
    for update in updates {
        match update {
            SweepUpdate::Status(status) => println!("· {status}"),
            SweepUpdate::Activated(flow) => println!("⚡ {flow}"),
            SweepUpdate::Record(record) => {
                let direction = match record.direction {
                    sc2_core::native::inspect::Direction::Incoming => "S→C",
                    sc2_core::native::inspect::Direction::Outgoing => "C→S",
                };
                println!(
                    "{direction} #{:04} {}/{} {:>6} B  {}",
                    record.sequence,
                    record.service,
                    record.command_id,
                    record.bytes.len(),
                    record.command
                );
            }
        }
    }
}

fn print_usage() {
    println!(
        "scanner-sweep\n\n  live [--output <capture.pcapng>] [--protocol-executable <SC2>] [--no-launch]\n  replay --pcap <capture> --game-utilities <jsonl> [--protocol-executable <SC2>] [--headless]\n\nLive capture uses Apple's tcpdump for PKTAP and Intel LLDB for the Rosetta-aware GameUtilities hook. Build for x86_64-apple-darwin and run as your macOS user; scanner-sweep elevates only tcpdump."
    );
}
