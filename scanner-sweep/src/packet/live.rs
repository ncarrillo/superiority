use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::OpenOptionsExt, process::ExitStatusExt},
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
    time::Duration,
};

use pcap_file::pcapng::{Block, PcapNgReader};

use super::{TcpPacket, parse_link_packet};
use crate::{Error, Result};

const METADATA_FILTER: &str =
    "proc = SC2 || eproc = SC2 || proc = SC2Switcher || eproc = SC2Switcher";

pub fn authorize() -> Result<()> {
    let status = Command::new("/usr/bin/sudo")
        .arg("-v")
        .status()
        .map_err(|error| {
            Error::Capture(format!("could not request packet-capture access: {error}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Capture(
            "administrator authentication for packet capture failed".to_owned(),
        ))
    }
}

pub fn run(
    output: &std::path::Path,
    stop: &AtomicBool,
    packets: &Sender<TcpPacket>,
    ready: &Sender<std::result::Result<(), String>>,
) -> Result<()> {
    let mut capture = match TcpdumpCapture::start() {
        Ok(capture) => capture,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return Err(error);
        }
    };
    if let Err(error) = capture.wait_until_ready() {
        let _ = ready.send(Err(error.to_string()));
        return Err(error);
    }
    let file = match OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(output)
    {
        Ok(file) => file,
        Err(error) => {
            let error = Error::Io(error);
            let _ = ready.send(Err(error.to_string()));
            return Err(error);
        }
    };
    let stdout = capture.take_stdout()?;
    let _ = ready.send(Ok(()));

    let finished = AtomicBool::new(false);
    let tcpdump_pid = capture.pid();
    let stream_result = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !finished.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            if stop.load(Ordering::Relaxed) && !finished.load(Ordering::Relaxed) {
                interrupt(tcpdump_pid);
            }
        });
        let result = stream_capture(stdout, file, stop, packets);
        finished.store(true, Ordering::Relaxed);
        result
    });
    let requested_stop = stop.load(Ordering::Relaxed);
    let process_result = capture.finish();
    stream_result?;
    process_result?;
    if requested_stop {
        Ok(())
    } else {
        Err(Error::Capture(
            "tcpdump ended before scanner-sweep stopped the capture".to_owned(),
        ))
    }
}

fn stream_capture(
    stdout: ChildStdout,
    file: File,
    stop: &AtomicBool,
    packets: &Sender<TcpPacket>,
) -> Result<()> {
    let stream = TeeReader::new(stdout, file);
    let mut reader = PcapNgReader::new(BufReader::new(stream))?;
    let mut interfaces = Vec::new();
    while let Some(block) = reader.next_block() {
        let block = match block {
            Ok(block) => block,
            Err(_) if stop.load(Ordering::Relaxed) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        match block {
            Block::SectionHeader(_) => interfaces.clear(),
            Block::InterfaceDescription(interface) => interfaces.push(interface.linktype),
            Block::EnhancedPacket(packet) => {
                let link = interfaces
                    .get(usize::try_from(packet.interface_id).map_err(|_| {
                        Error::Capture("pcapng interface id exceeds platform limits".to_owned())
                    })?)
                    .copied()
                    .ok_or_else(|| {
                        Error::Capture("pcapng packet references a missing interface".to_owned())
                    })?;
                if let Some(packet) = parse_link_packet(link, &packet.data)?
                    && packets.send(packet).is_err()
                {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct TcpdumpCapture {
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    status: Receiver<std::result::Result<(), String>>,
    log: Option<JoinHandle<String>>,
}

impl TcpdumpCapture {
    fn start() -> Result<Self> {
        let capture_user = std::env::var("USER")
            .ok()
            .filter(|user| !user.is_empty())
            .ok_or_else(|| Error::Capture("macOS did not provide USER".to_owned()))?;
        let mut child = Command::new("/usr/bin/sudo")
            .args(["-n", "/usr/sbin/tcpdump", "-i", "pktap,all"])
            .args(["-Q", METADATA_FILTER])
            .args(["-Z", &capture_user])
            .args(["-P", "-s", "0", "-B", "4096", "-U", "-v"])
            .args(["-w", "-", "tcp port 1119"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Error::Capture(format!("could not launch tcpdump: {error}")))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            Error::Capture("tcpdump did not expose its capture stream".to_owned())
        })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Capture("tcpdump did not expose its status stream".to_owned()))?;
        let (status_sender, status) = mpsc::channel();
        let log = std::thread::spawn(move || {
            let mut output = String::new();
            let mut announced = false;
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => {
                        if !announced && line.contains("listening on pktap") {
                            announced = true;
                            let _ = status_sender.send(Ok(()));
                        }
                        output.push_str(&line);
                        output.push('\n');
                    }
                    Err(error) => {
                        if !announced {
                            let _ = status_sender.send(Err(error.to_string()));
                        }
                        break;
                    }
                }
            }
            if !announced {
                let message = if output.is_empty() {
                    "tcpdump exited before opening pktap".to_owned()
                } else {
                    output.trim().to_owned()
                };
                let _ = status_sender.send(Err(message));
            }
            output
        });
        Ok(Self {
            child: Some(child),
            stdout: Some(stdout),
            status,
            log: Some(log),
        })
    }

    fn wait_until_ready(&self) -> Result<()> {
        match self.status.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(Error::Capture(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(Error::Capture(
                "timed out while tcpdump created the pktap interface".to_owned(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Capture(
                "tcpdump exited before opening pktap".to_owned(),
            )),
        }
    }

    fn take_stdout(&mut self) -> Result<ChildStdout> {
        self.stdout
            .take()
            .ok_or_else(|| Error::Capture("tcpdump capture stream was already taken".to_owned()))
    }

    fn pid(&self) -> u32 {
        self.child.as_ref().expect("tcpdump remains active").id()
    }

    fn finish(mut self) -> Result<()> {
        self.interrupt();
        let status = self.child.take().expect("tcpdump remains active").wait()?;
        let log = self
            .log
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default();
        if status.success() || status.signal() == Some(libc::SIGINT) {
            Ok(())
        } else {
            Err(Error::Capture(format!(
                "tcpdump exited with {status}: {}",
                log.trim()
            )))
        }
    }

    fn interrupt(&self) {
        if let Some(child) = self.child.as_ref() {
            interrupt(child.id());
        }
    }
}

impl Drop for TcpdumpCapture {
    fn drop(&mut self) {
        self.interrupt();
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
        if let Some(thread) = self.log.take() {
            let _ = thread.join();
        }
    }
}

fn interrupt(pid: u32) {
    if let Ok(pid) = i32::try_from(pid) {
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }
}

struct TeeReader<R, W> {
    source: R,
    destination: W,
}

impl<R, W> TeeReader<R, W> {
    fn new(source: R, destination: W) -> Self {
        Self {
            source,
            destination,
        }
    }
}

impl<R: Read, W: Write> Read for TeeReader<R, W> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let length = self.source.read(buffer)?;
        self.destination.write_all(&buffer[..length])?;
        Ok(length)
    }
}
